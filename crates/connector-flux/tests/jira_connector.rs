//! The Jira connector, and the one decision that shaped it: **Atlassian Document Format is not
//! expressible here, so this connector speaks the API version whose fields are plain.**
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims specific to Jira, because they are the reasons C-70 looks
//! the way it does and the reasons a later reader must not "modernise" the file:
//!
//! - **ADF cannot be declared.** A Jira v3 comment body is a nested rich-text document whose
//!   `content` is an *array* of block nodes. C-29's `wire` paths describe nested **objects** only —
//!   `connector_flux`'s `BodyNode` is `Leaf` or `Branch(BTreeMap)` and has no array variant — so an
//!   ADF document cannot be assembled at any `wire` path. The two remaining spellings are both
//!   refused by the story: a `body` typed `string` (which v3 rejects outright) or a free-form
//!   `object` the caller has to guess the schema of. So this connector addresses `/rest/api/2/`,
//!   where the vendor documents the same operations with a **plain string** comment body.
//!   [`no_jira_body_field_is_a_rich_text_document`] is what stops ADF being smuggled back in.
//! - **No query parameter, of any type.** Jira's `fields`, `expand`, `startAt` and `maxResults` are
//!   all query values, and the emitter interpolates a query value into the URL with no
//!   percent-encoding whatsoever (`crates/connector-flux/src/op.rs`, C-30). That is the defect
//!   `zendesk-ticket-search` carries. So the query surface is empty, asserted over the IR *and* over
//!   the emitted text — and over the **whole** module text, because the emitter re-binds `$url` once
//!   per optional query parameter inside a `when` guard, so reading only the first `$url` line would
//!   prove nothing.
//! - **No optional body field.** `body_tree` places every declared field into the payload record
//!   unconditionally, so a `required = false` body field the caller omits travels as an explicit
//!   `{"field": null}` (C-56). Jira is not documented as accepting null for the fields left out, so
//!   only always-sent fields are declared.
//!
//! Asserting these rather than trusting them is the point: nothing else in the repository would fail
//! if someone added an `expand` filter or moved to v3, and the resulting connector would look richer
//! and be broken.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod, Idempotency, Risk};

/// The provider under test. Named once so the file reads as being about Jira rather than a string.
const PROVIDER: &str = "jira";

/// The credential the connector declares, and the two environment variables it resolves from. All
/// three are public contract — an operator sets the variables and a manifest names the credential —
/// so they are pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "jira.api_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "JIRA_API_TOKEN";
/// See [`CREDENTIAL`]. The non-secret identity half: the Atlassian account email.
const USER_ENV: &str = "JIRA_USER";

/// The curated operations, in the order `providers/jira.toml` declares them.
const OPERATIONS: &[&str] = &[
    "jira-issue-get",
    "jira-issue-create",
    "jira-issue-comment-list",
    "jira-issue-comment-add",
    "jira-issue-transitions-list",
    "jira-issue-transition",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Every operation's emitted module text, paired with its operation id.
fn emitted(connector: &Connector) -> Vec<(String, String)> {
    connector
        .operations
        .iter()
        .map(|operation| {
            let text = emit_operation(connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            (operation.id.clone(), text)
        })
        .collect()
}

/// The connector exists, loads, and is the one the story specifies: Basic auth over a per-site
/// Atlassian host, with the curated operation set.
#[test]
fn the_jira_connector_loads_and_authenticates_with_an_email_and_api_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Jira");
    assert_eq!(connector.base_url, "https://{site}.atlassian.net");

    assert_eq!(
        connector.auth.len(),
        1,
        "jira authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("jira declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Basic,
        "Atlassian's API-token auth is HTTP Basic: the account email is the user half and the token \
         is the password"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert_eq!(method.user_env, [USER_ENV]);
    // **The halves must not be swapped.** The API token is the secret and must be the one gated and
    // redacted; the email is ordinary config. Getting this backwards routes a live credential
    // through the non-secret path, which is the regression C-19's acceptance names.
    assert!(
        method.user_suffix.is_none(),
        "jira's user half is the bare account email — zendesk's `/token` marker is Zendesk syntax \
         and Atlassian has no equivalent"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    // Every operation resolves to the one credential, whether it declares auth or inherits it.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; jira is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the API token",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; the Authorization header is injected \
             by the host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// Every operation is addressed by issue key on the `/rest/api/2/issue` resource tree.
///
/// **`/rest/api/2/`, not `/rest/api/3/`, and that is the ADF decision** — see the module docs. The
/// two versions offer the same collection of operations; v3's difference is that comment and
/// description bodies are ADF, which nothing here can express. Pinning the version in a test is what
/// makes a later bulk upgrade to v3 fail loudly instead of shipping a `string` Jira rejects.
#[test]
fn every_jira_operation_is_addressed_by_issue_key_on_the_v2_issue_resource() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.path.starts_with("/rest/api/2/issue"),
            "operation `{}` has path `{}`; every curated operation lives on the v2 issue resource \
             tree, and v3 would make its comment body ADF (see the module docs)",
            operation.id,
            operation.path
        );
        assert!(
            !operation.path.contains("/rest/api/3/"),
            "operation `{}` addresses API v3, whose comment and description bodies are Atlassian \
             Document Format — a nested document with an array-bearing `content`, which C-29's \
             `wire` paths cannot express at all",
            operation.id
        );
    }

    // The issue-scoped operations interpolate the vendor's own `{issueIdOrKey}` placeholder; create
    // is the one operation addressed at the collection.
    for (id, path) in [
        ("jira-issue-get", "/rest/api/2/issue/{issueIdOrKey}"),
        ("jira-issue-create", "/rest/api/2/issue"),
        (
            "jira-issue-comment-list",
            "/rest/api/2/issue/{issueIdOrKey}/comment",
        ),
        (
            "jira-issue-comment-add",
            "/rest/api/2/issue/{issueIdOrKey}/comment",
        ),
        (
            "jira-issue-transitions-list",
            "/rest/api/2/issue/{issueIdOrKey}/transitions",
        ),
        (
            "jira-issue-transition",
            "/rest/api/2/issue/{issueIdOrKey}/transitions",
        ),
    ] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("jira declares `{id}`"));
        assert_eq!(operation.path, path, "`{id}` addresses the wrong resource");
    }
}

/// **The load-bearing assertion: no query parameters at all.**
///
/// The story's requirement is that no operation declares a string-ish or `Any`-typed query
/// parameter. This asserts the stronger and simpler property that implies it — the query surface is
/// empty — because "empty" is a claim that stays true under editing, whereas "string-ish" invites a
/// later author to add a numeric `maxResults` and rebuild the query-assembly path this connector
/// exists to avoid.
#[test]
fn no_jira_operation_declares_a_query_parameter() {
    let connector = load();

    for operation in &connector.operations {
        let declared: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            declared.is_empty(),
            "operation `{}` declares query parameters {declared:?}. Jira's `fields`, `expand`, \
             `startAt` and `maxResults` are all query values, and a query value is interpolated \
             into the URL with no percent-encoding (C-30) — the defect behind \
             `zendesk-ticket-search`",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads.
///
/// The IR assertion above and this one are not the same check. The emitter assembles a query string
/// from `params.query` alone, binding `sep` to carry the `?`/`&` separator; if that machinery
/// appears in a Jira module, something put a value in the URL regardless of what the IR looked like
/// when the previous test ran.
///
/// The scan is over the **whole** module text on purpose: a required query parameter lands in the
/// first `$url` template, but an optional one is emitted as a *second* `$url` binding inside a
/// `when` guard, so checking only the first line would miss exactly the parameter the guard is for.
#[test]
fn no_jira_module_assembles_a_query_string() {
    let connector = load();

    for (id, text) in emitted(&connector) {
        assert!(
            !text.contains("sep = "),
            "`{id}` emits query-string separator machinery, so a value is reaching the URL \
             unencoded:\n{text}"
        );
        assert!(
            !text.contains('?'),
            "`{id}` emits a `?` in its request URL:\n{text}"
        );
        // The guard the emitter wraps an optional query parameter in. Its absence is the second
        // half of the proof: no re-bound `$url` survives anywhere in the module.
        assert_eq!(
            text.matches("url = ").count(),
            1,
            "`{id}` binds `$url` more than once, which is how the emitter re-binds it per optional \
             query parameter inside a `when` guard:\n{text}"
        );
    }
}

/// **No optional request-body field, until C-56 lands.**
///
/// `body_tree` places every declared body field into the payload record unconditionally — there is
/// no `when` guard on the body side, unlike the query side — so a `required = false` body field the
/// caller omits travels to Jira as an explicit `{"field": null}`. Jira is not documented as treating
/// null as absent for the fields C-70 left out, and AGENTS.md says to refuse plausible-but-unverified
/// behaviour rather than ship it.
#[test]
fn no_jira_operation_declares_an_optional_body_field() {
    let connector = load();

    for operation in &connector.operations {
        for param in &operation.params.body {
            assert!(
                param.required,
                "operation `{}` declares optional body field `{}`; an omitted optional body field \
                 is emitted as an explicit null (C-56), so only always-sent fields may be declared",
                operation.id, param.name
            );
        }
    }
}

/// **Atlassian Document Format stays out, and so does every other guessable body shape.**
///
/// This is the assertion that holds the story's ADF decision in place. Every declared body field is
/// a JSON *scalar* — `string`, `integer`, `number` or `boolean` — so no field is a nested rich-text
/// document, and no operation declares a free-form `body_schema` the caller would have to fill in
/// blind.
///
/// Both failure modes it excludes are ones that *look* like they work:
///
/// - a v3 comment body typed `string` is accepted by the schema here and rejected by Jira;
/// - a body field typed `object` (or a `body_schema`) compiles, and hands a model an argument whose
///   shape it has to invent — for ADF that means inventing `{"type": "doc", "version": 1,
///   "content": [...]}` correctly, on every call.
///
/// The structural reason ADF cannot be declared honestly is in the module docs: `BodyNode` has no
/// array variant, so `content`'s array of block nodes has no `wire` spelling.
#[test]
fn no_jira_body_field_is_a_rich_text_document() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`; Jira's bodies are known, and a \
             free-form body here would be where an ADF document was smuggled in for a model to \
             guess at",
            operation.id
        );

        for param in &operation.params.body {
            let declared = param.schema.get("type").and_then(|value| value.as_str());
            assert!(
                matches!(declared, Some("string" | "integer" | "number" | "boolean")),
                "operation `{}` declares body field `{}` with type {declared:?}; every body field \
                 must be a scalar. An `object` or `array` here is how Atlassian Document Format \
                 would arrive — and `wire` paths cannot express its array-bearing `content` \
                 anyway (C-29)",
                operation.id,
                param.name
            );
        }
    }

    // The positive half: the comment body really is the plain string v2 documents, at the root of
    // the body rather than nested under an ADF document node.
    let comment_add = connector
        .operations
        .iter()
        .find(|operation| operation.id == "jira-issue-comment-add")
        .expect("jira declares `jira-issue-comment-add`");
    let body = comment_add
        .params
        .body
        .iter()
        .find(|param| param.name == "body")
        .expect("`jira-issue-comment-add` sends a `body` field");
    assert_eq!(
        body.schema.get("type").and_then(|v| v.as_str()),
        Some("string")
    );
    assert!(
        body.wire.is_none(),
        "v2 takes the comment text at the body root as `body`; a `wire` path here would mean the \
         ADF nesting crept back in"
    );
}

/// Write metadata says what each write changes and who sees it, and no write claims to be
/// idempotent.
///
/// The emitter already refuses the unsafe direction — a mutating method declaring `risk = "low"`, or
/// a `POST` declaring itself idempotent. This pins the *positive* claim per operation, which the
/// emitter cannot check: that the three reads are reads, and that each of the three writes is
/// recorded `high` because it is visible to a whole project and notifies its watchers.
#[test]
fn every_jira_write_declares_what_it_changes_and_never_claims_idempotence() {
    let connector = load();

    for operation in &connector.operations {
        let mutates = matches!(operation.method, HttpMethod::Post);
        if mutates {
            assert_ne!(
                operation.risk,
                Risk::Low,
                "operation `{}` is a write declared low risk",
                operation.id
            );
            assert_ne!(
                operation.idempotency,
                Idempotency::Idempotent,
                "operation `{}` is a POST claiming idempotence, which makes a `retry` around it \
                 unsound",
                operation.id
            );
        } else {
            assert_eq!(
                operation.method,
                HttpMethod::Get,
                "operation `{}` uses a method this connector does not curate",
                operation.id
            );
            assert_eq!(
                operation.risk,
                Risk::Low,
                "operation `{}` is a GET; a read of one issue is low risk",
                operation.id
            );
            assert_eq!(
                operation.idempotency,
                Idempotency::Idempotent,
                "operation `{}` is a GET, which is repeatable",
                operation.id
            );
        }
    }

    for id in [
        "jira-issue-create",
        "jira-issue-comment-add",
        "jira-issue-transition",
    ] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("jira declares `{id}`"));
        assert_eq!(
            operation.risk,
            Risk::High,
            "`{id}` is visible to everyone with access to the project and notifies the issue's \
             watchers"
        );
        assert_eq!(operation.idempotency, Idempotency::NonIdempotent);
    }
}

/// **The tenant template is recorded as unbound, and the egress host is never widened.**
///
/// `{site}` is a placeholder no field binds to an environment variable, exactly as zendesk's
/// `{subdomain}` is — `crates/connector-cli/src/status.rs` publishes that as
/// `unbound-base-url-template`, and C-68 is where the binding is designed. What this pins is that
/// the template is the *only* thing unresolved: the host is a single Atlassian domain and not a
/// wildcard, so the derived `http_hosts` allow-list cannot be widened to `*` by editing this file.
#[test]
fn the_tenant_template_is_unbound_and_the_host_is_never_a_wildcard() {
    let connector = load();

    assert!(
        connector.base_url.contains("{site}"),
        "jira's base URL must carry the tenant template an operator binds: {}",
        connector.base_url
    );
    assert!(
        connector.base_url.ends_with(".atlassian.net"),
        "every Jira Cloud site is a subdomain of atlassian.net: {}",
        connector.base_url
    );
    assert!(
        !connector.base_url.contains('*'),
        "a wildcard in the base URL would widen the derived egress allow-list to every host: {}",
        connector.base_url
    );

    // The template reaches the emitted module verbatim, unresolved — the connector cannot invent a
    // site, and a build must not silently substitute one.
    for (id, text) in emitted(&connector) {
        assert!(
            text.contains(r#"base = "https://{site}.atlassian.net""#),
            "`{id}` does not carry the unbound tenant template:\n{text}"
        );
    }
}

/// **No credential reaches a generated module** — not a value, and not even a variable name.
///
/// Auth injection is C-10 and is deliberately absent from emitted Flux rather than stubbed, so the
/// strongest available statement is that nothing credential-shaped is in the text at all. A `$secret`
/// marker or a literal `JIRA_API_TOKEN` appearing here would mean the emitter had started resolving
/// or naming credentials in generated output, which is the boundary AGENTS.md draws.
#[test]
fn no_jira_module_carries_a_credential_or_its_variable_name() {
    let connector = load();

    for (id, text) in emitted(&connector) {
        for forbidden in [TOKEN_ENV, USER_ENV, CREDENTIAL, "$secret", "Authorization"] {
            assert!(
                !text.contains(forbidden),
                "`{id}` names `{forbidden}` in generated Flux; a generated module carries no \
                 credential and no credential reference (C-10, AGENTS.md):\n{text}"
            );
        }
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and **loads** as exactly one exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set. It is restated here so that the
/// Jira connector's own test file fails on its own when the module stops being analyzable — a
/// provider whose emitted Flux does not load publishes no ops at all, and a consumer handing it to
/// flux would get silence rather than an error.
#[test]
fn every_jira_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

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
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
        assert!(
            program.ops[0].meta.expose,
            "`{}` must be exposed to the model as a tool",
            operation.id
        );
    }
}
