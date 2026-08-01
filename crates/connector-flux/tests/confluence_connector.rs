//! The Confluence connector, and the probe it exists to answer: **two connectors, one Atlassian
//! account, one API token — does the second one resolve to the credential the first already has?**
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims specific to Confluence, and the first of them is not
//! about Confluence at all — it is about the credential-addressing model (C-90).
//!
//! # The probe, and its answer
//!
//! `providers/jira.toml` and `providers/confluence.toml` describe the *same account on the same
//! host*: `{site}.atlassian.net`, HTTP Basic, `<account email>:<API token>`, one token minted once
//! at id.atlassian.com. C-90's premise is that a credential address is a **place**, not a
//! per-connector copy. So the question this story was filed to settle is whether an operator who has
//! connected Jira must paste that token a second time for Confluence.
//!
//! **The measured answer is yes, they must.** [`the_two_atlassian_connectors_do_not_share_a_credential_address`]
//! renders both addresses and asserts they differ:
//!
//! ```text
//! tenants/<tenant>/com.atlassian.jira/api_token
//! tenants/<tenant>/com.atlassian.confluence/api_token
//! ```
//!
//! Two sibling directories, sharing nothing. The model is not broken — it is doing exactly what it
//! says — but the sharing C-90's prose implies **does not fall out of it**, and nothing before this
//! story had two connectors close enough together to show that. The reason is structural and worth
//! stating precisely: `Connector::credential_ref_for` (`ir.rs:1166-1178`) keys an address on the
//! **`authority` plus the credential leaf**, and an `authority` is derived per *product* by the C-92
//! rule `providers/jira.toml` records. Two products never share one, so two connectors never share a
//! credential — no matter how identical the credential physically is.
//!
//! This test asserts that *as the current answer*, not as a desirable one. If a later story gives a
//! connector a way to declare that its credential is the same place as another's, this test is the
//! one that must be rewritten, deliberately, with the migration in hand — because every tenant that
//! provisioned under the address above would have to move.
//!
//! # Why a standalone connector, and not a service on an `atlassian` one
//!
//! C-49 established services for exactly this shape, and the alternative was live: one `atlassian`
//! connector with `jira` and `confluence` services, one `[[auth]]` credential, one address —
//! `tenants/<tenant>/com.atlassian/api_token` — and the operator pastes once.
//! [`a_service_split_would_have_shared_the_credential_and_is_still_refused`] constructs that
//! hypothetical address and shows it *would* have worked, so the decision is recorded as a
//! trade-off rather than as an absence.
//!
//! It is refused because the cost is paid by tenants, not by this repository. `com.atlassian.jira`
//! is a **published** address, and AGENTS.md's service contract is unambiguous: *"An address, once
//! published, is not reused."* Moving Jira under an `atlassian` connector repoints every Jira
//! address, every `gid`, and every credential path a tenant has already provisioned. That is a
//! migration story with a deprecation window, and it is not this one — which is a provider story
//! that must leave `providers/jira.toml` untouched.
//!
//! # What this connector cannot do, and why it is small
//!
//! **No operation declares a query parameter** (C-30: the emitter interpolates query values into the
//! URL and percent-encodes nothing — the `zendesk-ticket-search` defect). On Confluence v2 that
//! single constraint has an unusually sharp consequence, because `body-format` is a *query*
//! parameter and omitting it returns `"body": {}`:
//!
//! **this connector cannot read any content body — not a page's, not a comment's.**
//!
//! So it is curated as a connector that *navigates and writes* a Confluence site rather than one
//! that reads it back. [`no_confluence_operation_declares_a_query_parameter`] and
//! [`no_confluence_module_assembles_a_query_string`] hold that line on the declaration and on the
//! emitted Flux, and the provider header names every endpoint left out for it.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{
    provider, AuthScheme, Connector, CredentialRef, HttpMethod, Idempotency, Layout, Risk,
    TenantInstances, TenantLayout,
};

/// The provider under test.
const PROVIDER: &str = "confluence";

/// The sibling connector this story's probe is *about*. Loaded read-only, never modified.
const SIBLING: &str = "jira";

/// The credential the connector declares, and the two environment variables it resolves from. A
/// variable *name*; no credential value appears in this repository.
const CREDENTIAL: &str = "confluence.api_token";
/// See [`CREDENTIAL`]. The secret half — the Atlassian API token.
const TOKEN_ENV: &str = "CONFLUENCE_API_TOKEN";
/// See [`CREDENTIAL`]. The non-secret identity half: the Atlassian account email.
const USER_ENV: &str = "CONFLUENCE_USER";

/// A syntactically valid tenant id, used to render an address. Not a real tenant.
const TENANT: &str = "9f3a4b2c";

/// The curated operations, in the order `providers/confluence.toml` declares them.
const OPERATIONS: &[&str] = &[
    "confluence-space-list",
    "confluence-space-get",
    "confluence-space-pages",
    "confluence-page-get",
    "confluence-page-create",
    "confluence-comment-add",
];

/// The argument-free read that proves the arrangement works.
const VERIFY: &str = "confluence-space-list";

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load_provider(id: &str) -> Connector {
    let path = providers_dir().join(format!("{id}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    provider::load(&format!("providers/{id}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{id}.toml does not load: {error}"))
        .connector
}

fn load() -> Connector {
    load_provider(PROVIDER)
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

/// **THE PROBE. Two Atlassian connectors, one physical token, two credential addresses.**
///
/// This is the assertion the story was filed for, and it is deliberately written as a comparison
/// between the two shipped provider files rather than as a claim about Confluence alone — the whole
/// question is relational.
///
/// The two connectors describe the same account on the same host with the same token. They
/// nevertheless resolve to *different* places, because `credential_ref_for` keys an address on the
/// authority, and the C-92 authority rule assigns a product-level authority to each. An operator who
/// has connected Jira **must supply the token again** for Confluence.
///
/// Both halves are asserted: that the addresses differ, and *why* — same tenant, same leaf name,
/// differing only in the authority segment. A future change that made them equal would flip the
/// first assertion; a future change that made them differ for some *other* reason (a renamed leaf,
/// say) would flip the second, and would be a different fact wearing this one's clothes.
#[test]
fn the_two_atlassian_connectors_do_not_share_a_credential_address() {
    let confluence = load();
    let jira = load_provider(SIBLING);

    // The premise: these really are the same credential in the world. If this half ever stops
    // holding, the probe below is answering a question nobody asked.
    let confluence_method = confluence
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("confluence declares `{CREDENTIAL}`"));
    let jira_method = jira
        .auth_method("jira.api_token")
        .expect("jira declares `jira.api_token`");
    assert_eq!(
        confluence_method.scheme,
        AuthScheme::Basic,
        "Confluence Cloud REST authenticates with HTTP Basic, the account email against the API token"
    );
    assert_eq!(
        confluence_method.scheme, jira_method.scheme,
        "the probe is only meaningful while both connectors authenticate the same way"
    );
    // The same host, differing only in the path prefix Confluence occupies on it (`/wiki`). This is
    // what makes the two credentials one credential in the world, and therefore what makes the
    // address comparison below a real question rather than a coincidence of naming.
    const ATLASSIAN_HOST: &str = "https://{site}.atlassian.net";
    assert_eq!(jira.base_url, ATLASSIAN_HOST);
    assert_eq!(confluence.base_url, format!("{ATLASSIAN_HOST}/wiki"));

    let confluence_ref = confluence
        .credential_ref_for(TENANT, CREDENTIAL, TenantInstances::sole())
        .expect("a valid tenant id and a declared credential must resolve")
        .expect("confluence declares an authority, so a reference must render");
    let jira_ref = jira
        .credential_ref_for(TENANT, "jira.api_token", TenantInstances::sole())
        .expect("a valid tenant id and a declared credential must resolve")
        .expect("jira declares an authority, so a reference must render");

    let confluence_path = TenantLayout.render(&confluence_ref);
    let jira_path = TenantLayout.render(&jira_ref);

    // ---- THE ANSWER -------------------------------------------------------------------------
    assert_ne!(
        confluence_path, jira_path,
        "THE PROBE'S ANSWER: an operator who connected Jira must paste the same Atlassian API \
         token again for Confluence. C-90's premise that an address is a place rather than a \
         per-connector copy is true *within* a connector and does not reach across two, because \
         `credential_ref_for` keys on the authority and C-92 derives an authority per product."
    );
    assert_eq!(
        confluence_path,
        format!("tenants/{TENANT}/com.atlassian.confluence/api_token")
    );
    assert_eq!(
        jira_path,
        format!("tenants/{TENANT}/com.atlassian.jira/api_token")
    );

    // ---- AND WHY ----------------------------------------------------------------------------
    // Same tenant, same leaf. The authority segment is the *only* difference, which is what makes
    // this a fact about the addressing model rather than about how these two files were named.
    assert_eq!(confluence_ref.tenant(), jira_ref.tenant());
    assert_eq!(
        confluence_ref.credential(),
        jira_ref.credential(),
        "both leaves are `api_token` — the vendor prefix is dropped (`local_credential_name`), so \
         the leaf cannot be what separates them"
    );
    assert_eq!(confluence_ref.credential(), "api_token");
    assert_ne!(
        confluence_ref.authority(),
        jira_ref.authority(),
        "the authority is the whole of the difference"
    );
    assert_eq!(confluence_ref.authority(), "com.atlassian.confluence");
    assert_eq!(jira_ref.authority(), "com.atlassian.jira");

    // Neither carries a service segment: `credential_ref_for` renders the elided default service for
    // every connector today, so a service split is not what separated these two either.
    assert!(confluence_ref.is_default_service() && jira_ref.is_default_service());
    assert_eq!(confluence_path.split('/').count(), 4);
}

/// **The road not taken, measured: a service split would in fact have shared the credential.**
///
/// The decision between a standalone `confluence` connector and a second *service* on an
/// `atlassian` connector is recorded in this file's module docs and in the provider header. This
/// test exists so the rejected branch is not merely asserted to have been considered — the
/// hypothetical address is constructed and shown to be *one* address for both products.
///
/// `credential_ref_for` renders the elided default service regardless of which service an operation
/// belongs to (`ir.rs:1153-1160`, and `postmark_connector.rs` measures the same thing). So one
/// `atlassian` connector declaring one `atlassian.api_token` would produce exactly one place, and
/// the operator would paste once. The reason that is still refused is not technical: it repoints
/// `com.atlassian.jira`, which is published, and AGENTS.md forbids reusing a published address.
#[test]
fn a_service_split_would_have_shared_the_credential_and_is_still_refused() {
    // What one `atlassian` connector with `jira` and `confluence` services would have addressed.
    let unified = CredentialRef::new(TENANT, "com.atlassian", "default", "api_token")
        .expect("a hypothetical unified Atlassian authority is a valid address");
    let unified_path = TenantLayout.render(&unified);
    assert_eq!(
        unified_path,
        format!("tenants/{TENANT}/com.atlassian/api_token"),
        "the rejected design really would have been one place for both products"
    );

    // And it is a place *neither* shipped connector resolves to, which is what makes the choice a
    // trade-off with a cost rather than a free win forgone.
    let confluence_path = TenantLayout.render(
        &load()
            .credential_ref_for(TENANT, CREDENTIAL, TenantInstances::sole())
            .expect("resolves")
            .expect("renders"),
    );
    let jira_path = TenantLayout.render(
        &load_provider(SIBLING)
            .credential_ref_for(TENANT, "jira.api_token", TenantInstances::sole())
            .expect("resolves")
            .expect("renders"),
    );
    assert_ne!(unified_path, confluence_path);
    assert_ne!(unified_path, jira_path);

    // The shipped Jira authority is the thing a service split would have had to repoint, and this
    // story does not touch `providers/jira.toml`. Pinning it here makes that explicit: if a later
    // migration does move it, this test is one of the places that must be revisited on purpose.
    assert_eq!(
        load_provider(SIBLING).authority.as_deref(),
        Some("com.atlassian.jira"),
        "jira's published authority is unchanged by this story"
    );
}

/// The connector exists, loads, and is the one the story specifies.
#[test]
fn the_confluence_connector_loads_and_authenticates_with_an_email_and_api_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Confluence");
    assert_eq!(connector.base_url, "https://{site}.atlassian.net/wiki");
    assert_eq!(
        connector.authority.as_deref(),
        Some("com.atlassian.confluence")
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "confluence authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("confluence declares `{CREDENTIAL}`"));
    assert_eq!(method.scheme, AuthScheme::Basic);
    assert_eq!(method.env, [TOKEN_ENV]);
    assert_eq!(method.user_env, [USER_ENV]);
    // **The halves must not be swapped.** The API token is the secret and must be the one gated and
    // redacted; the email is ordinary config. Getting this backwards routes a live credential
    // through the non-secret `user_env` path — the regression C-19's acceptance names, and the same
    // assertion `jira_connector.rs` carries.
    assert!(
        method.user_suffix.is_none(),
        "Atlassian's user half is the bare account email; Zendesk's `/token` marker is Zendesk \
         syntax and Atlassian has no equivalent"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; confluence is single-mechanism",
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

/// Every operation is on the v2 API, under the `/wiki` base path.
///
/// Confluence Cloud also serves a legacy `/wiki/rest/api` surface (v1). This connector is written
/// against v2 throughout, and pinning it here makes a partial migration in either direction fail
/// loudly rather than producing a file that addresses two APIs at once.
#[test]
fn every_confluence_operation_is_on_the_v2_api_under_the_wiki_base_path() {
    let connector = load();

    assert!(
        connector.base_url.ends_with("/wiki"),
        "the `/wiki` prefix is Confluence's on the shared Atlassian host and belongs to the base \
         URL, not to each path: {}",
        connector.base_url
    );
    assert_eq!(connector.api_version.as_deref(), Some("v2"));

    for operation in &connector.operations {
        assert!(
            !operation.path.contains("/rest/api"),
            "operation `{}` addresses the legacy v1 surface (`/wiki/rest/api`): {}",
            operation.id,
            operation.path
        );
        assert!(
            operation.path.starts_with("/api/v2/"),
            "operation `{}` is not on the v2 API: {}",
            operation.id,
            operation.path
        );
    }

    for (id, path) in [
        ("confluence-space-list", "/api/v2/spaces"),
        ("confluence-space-get", "/api/v2/spaces/{id}"),
        ("confluence-space-pages", "/api/v2/spaces/{id}/pages"),
        ("confluence-page-get", "/api/v2/pages/{id}"),
        ("confluence-page-create", "/api/v2/pages"),
        ("confluence-comment-add", "/api/v2/footer-comments"),
    ] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("confluence declares `{id}`"));
        assert_eq!(operation.path, path, "`{id}` addresses the wrong resource");
    }
}

/// **No query parameter, of any type — and on this vendor that is what costs the content bodies.**
///
/// The same assertion `jira_connector.rs` carries, for the same reason (C-30). What is specific to
/// Confluence is the size of the consequence: `body-format` is a query parameter, and a v2 read that
/// omits it returns `"body": {}`. So this connector reads no page content and no comment content,
/// and the provider header says so in those words.
///
/// The stronger, simpler property is asserted — the query surface is empty — because "empty" stays
/// true under editing, whereas a narrower rule invites a later author to add "just `body-format`"
/// and rebuild the query-assembly path this connector exists to avoid.
#[test]
fn no_confluence_operation_declares_a_query_parameter() {
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
            "operation `{}` declares query parameters {declared:?}. Confluence's `body-format`, \
             `cursor`, `limit`, `status` and every `include-*` flag are query values, and a query \
             value is interpolated into the URL with no percent-encoding (C-30)",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads.
///
/// Not the same check as the one above: the emitter assembles a query string from `params.query`
/// alone, binding `sep` to carry the `?`/`&` separator. The scan covers the whole module because an
/// optional query parameter is emitted as a *second* `$url` binding inside a `when` guard.
///
/// The `?` assertion has a second job here. Confluence's `body-format` is a *constant* the vendor
/// fixes, not a caller value, so the tempting workaround is to bake `?body-format=storage` into an
/// operation's `path` — a literal, with no encoding hazard, that would restore the page bodies.
/// It is refused (see the provider header), and this is what refuses it: no `?` reaches any emitted
/// URL by any route, whether through a declared parameter or through a hand-written path.
#[test]
fn no_confluence_module_assembles_a_query_string() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            !operation.path.contains('?'),
            "operation `{}` carries a literal query string in its `path`: {}. A constant query \
             parameter has no mechanism here (C-55 pinned constant *headers* and constant *body* \
             fields and stopped there), and smuggling one through the path would break the moment \
             a real query parameter was added beside it",
            operation.id,
            operation.path
        );
    }

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
        assert_eq!(
            text.matches("url = ").count(),
            1,
            "`{id}` binds `$url` more than once, which is how the emitter re-binds it per optional \
             query parameter inside a `when` guard:\n{text}"
        );
    }
}

/// **No optional request-body field, and every body nesting is an object rather than an array.**
///
/// C-56: `body_tree` places every declared field into the payload unconditionally, so a
/// `required = false` body field the caller omits travels as an explicit `{"field": null}`.
///
/// The array half is the Jira lesson applied to a sibling product. Confluence v2 write bodies nest
/// (`body.representation`, `body.value`, and on an update `version.number`), and C-29's `wire` paths
/// express nested **objects** only — `BodyNode` is `Leaf` or `Branch`, with no array variant. Every
/// path declared here is an object path, and every leaf is a scalar.
#[test]
fn every_confluence_body_field_is_a_required_scalar_at_an_object_path() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`; Confluence's write bodies are \
             known, and a free-form body is what AGENTS.md lists among the shapes to refuse rather \
             than hand a model to guess at",
            operation.id
        );

        for param in &operation.params.body {
            assert!(
                param.required,
                "operation `{}` declares optional body field `{}`; an omitted optional body field \
                 is emitted as an explicit null (C-56), so only always-sent fields may be declared",
                operation.id, param.name
            );
            let declared = param.schema.get("type").and_then(|value| value.as_str());
            assert!(
                matches!(declared, Some("string" | "integer" | "number" | "boolean")),
                "operation `{}` declares body field `{}` with type {declared:?}; every body field \
                 must be a scalar, because a `wire` path cannot express an array (C-29)",
                operation.id,
                param.name
            );
        }
    }
}

/// **The two vendor-fixed body values are pinned, not asked for.**
///
/// Confluence's write bodies carry two members whose value this connector always chooses:
/// `body.representation` is always `storage`, and a created page is always `current` rather than a
/// draft. Both are pinned with a JSON Schema `const`, which `op.rs` sends as a literal and
/// deliberately does **not** declare as a caller argument (`op.rs:486-494`).
///
/// This matters more than it looks. A caller-supplied `representation` would let a model send
/// `wiki` markup labelled `storage`, which Confluence stores verbatim and renders as broken markup
/// — a `200` and a corrupted page, not an error. Pinning removes the choice rather than documenting
/// it, and this test asserts the pin is really there on both writes.
#[test]
fn the_vendor_fixed_write_fields_are_pinned_and_never_caller_supplied() {
    let connector = load();

    for (id, field, value) in [
        ("confluence-page-create", "status", "current"),
        ("confluence-page-create", "body_representation", "storage"),
        ("confluence-comment-add", "body_representation", "storage"),
    ] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("confluence declares `{id}`"));
        let param = operation
            .params
            .body
            .iter()
            .find(|param| param.name == field)
            .unwrap_or_else(|| panic!("`{id}` declares body field `{field}`"));
        assert_eq!(
            param.schema.get("const").and_then(|v| v.as_str()),
            Some(value),
            "`{id}`'s `{field}` must be pinned to `{value}` with a JSON Schema `const`, so it is \
             sent as a literal and never becomes an argument a model can choose"
        );
    }

    // The pin's whole point is that the value does not reach the operation's declared signature.
    // Asserted on the emitted Flux, because that is the surface a model is handed.
    for (id, text) in emitted(&connector) {
        if id == "confluence-page-create" {
            assert!(
                !text.contains("status: String"),
                "`{id}` declares `status` as an argument; the `const` pin was dropped:\n{text}"
            );
        }
        if id == "confluence-page-create" || id == "confluence-comment-add" {
            assert!(
                !text.contains("body_representation: String"),
                "`{id}` declares `body_representation` as an argument; the `const` pin was \
                 dropped:\n{text}"
            );
            assert!(
                text.contains(r#""storage""#),
                "`{id}` must send the pinned representation as a literal:\n{text}"
            );
        }
    }
}

/// **No pagination quirk, because Confluence's cursor is doubly inexpressible — and the reads say
/// so in their own descriptions.**
///
/// The story asks for this to be stated rather than papered over. Confluence v2 pages with an opaque
/// cursor, and `Quirks::Pagination::Cursor` cannot carry it for two independent reasons:
///
/// 1. `cursor_param` is defined as a **query** parameter, and this connector declares none (C-30) —
///    the same wall `providers/notion.toml`, `providers/okta.toml` and `providers/intercom.toml`
///    each hit.
/// 2. `next_cursor_pointer` is a JSON Pointer to the **cursor value**, and Confluence's
///    `_links.next` holds a *relative URL* with the cursor embedded as a query parameter
///    (`/wiki/api/v2/pages?limit=25&cursor=…`), not a bare token. Even with the query surface open,
///    the pointer would locate a URL rather than a cursor — `providers/front.toml` records the same
///    shape and refuses it too.
///
/// So each list read returns Confluence's first default-sized page (25) and nothing else, and the
/// only honest place to say so is the description a model actually receives.
#[test]
fn no_confluence_operation_declares_a_pagination_quirk_and_every_list_read_says_so() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.quirks.pagination.is_none(),
            "operation `{}` declares a pagination quirk. Confluence's cursor is a query parameter \
             this connector cannot send, and `_links.next` is a relative URL rather than the bare \
             cursor `next_cursor_pointer` is defined to locate",
            operation.id
        );
    }

    // A list read that silently returns one page is exactly what the story says not to ship. The
    // truncation has to reach the tool contract, so it is asserted on the text a model is given.
    for id in ["confluence-space-list", "confluence-space-pages"] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("confluence declares `{id}`"));
        let description = operation.description.to_lowercase();
        assert!(
            description.contains("first page") || description.contains("25"),
            "`{id}` is a paginated read whose description does not tell a model it sees only the \
             first page: {:?}",
            operation.description
        );
    }
}

/// **No read claims to return content it cannot return.**
///
/// This is the assertion that keeps the `body-format` gap honest. `confluence-page-get` and
/// `confluence-space-pages` return page *metadata* with `"body": {}`, because `body-format` is a
/// query parameter (see [`no_confluence_operation_declares_a_query_parameter`]). A description that
/// promised page content would be a tool contract a model would rely on and the vendor would not
/// honour — the failure mode is an empty string presented as an empty page, not an error.
///
/// So every read that touches a page states the absence, and no response schema declares a
/// populated `body`.
#[test]
fn no_confluence_read_promises_page_content_it_cannot_fetch() {
    let connector = load();

    for id in ["confluence-page-get", "confluence-space-pages"] {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap_or_else(|| panic!("confluence declares `{id}`"));
        let description = operation.description.to_lowercase();
        assert!(
            description.contains("body"),
            "`{id}` reads pages and must state that the page body is not returned: {:?}",
            operation.description
        );
        assert!(
            description.contains("not returned")
                || description.contains("no page content")
                || description.contains("empty"),
            "`{id}` mentions the body but does not say it is absent: {:?}",
            operation.description
        );
    }
}

/// `verify` is an argument-free read that runs unattended.
///
/// AGENTS.md: *"A `verify` operation is a read."* It is the "Test connection" button and runs
/// whenever someone opens a settings page. The loader already refuses a `high` or `destructive`
/// one; what it does not check is that the operation takes **no arguments**, which is what makes it
/// runnable before anything but the credential has been configured.
#[test]
fn the_verify_operation_is_an_argument_free_read() {
    let connector = load();

    assert_eq!(connector.verify.as_deref(), Some(VERIFY));
    let operation = connector
        .operation(VERIFY)
        .unwrap_or_else(|| panic!("`{VERIFY}` is declared"));

    assert_eq!(operation.method, HttpMethod::Get);
    assert_eq!(operation.risk, Risk::Low);
    assert_eq!(operation.idempotency, Idempotency::Idempotent);
    assert!(
        operation.params.path.is_empty()
            && operation.params.query.is_empty()
            && operation.params.body.is_empty()
            && operation.params.header.is_empty(),
        "`{VERIFY}` takes arguments, so it cannot run unattended from a settings page"
    );
}

/// Write metadata says what each write changes, and no write claims to be idempotent.
#[test]
fn every_confluence_write_declares_what_it_changes_and_never_claims_idempotence() {
    let connector = load();

    for operation in &connector.operations {
        match operation.method {
            HttpMethod::Post => {
                assert_ne!(
                    operation.risk,
                    Risk::Low,
                    "operation `{}` is a write declared low risk",
                    operation.id
                );
                assert_ne!(
                    operation.idempotency,
                    Idempotency::Idempotent,
                    "operation `{}` is a POST claiming idempotence, which makes a `retry` around \
                     it unsound",
                    operation.id
                );
            }
            HttpMethod::Get => {
                assert_eq!(
                    operation.risk,
                    Risk::Low,
                    "operation `{}` is a GET; a read is low risk",
                    operation.id
                );
                assert_eq!(operation.idempotency, Idempotency::Idempotent);
            }
            other => panic!(
                "operation `{}` uses method {other:?}, which this connector does not curate — the \
                 update and delete endpoints are excluded on purpose (see the provider header)",
                operation.id
            ),
        }
    }
}

/// **No credential reaches a generated module** — not a value, and not even a variable name.
#[test]
fn no_confluence_module_carries_a_credential_or_its_variable_name() {
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

/// **The tenant template is unbound and the egress host is never widened.**
///
/// `{site}` is the same placeholder `providers/jira.toml` carries, and the same C-68 gap. What is
/// pinned here is that the template is the *only* unresolved part: the host is a single Atlassian
/// domain and not a wildcard, so the derived `http_hosts` allow-list cannot be widened by editing
/// this file.
#[test]
fn the_tenant_template_is_unbound_and_the_host_is_never_a_wildcard() {
    let connector = load();

    assert!(
        connector.base_url.contains("{site}"),
        "confluence's base URL must carry the tenant template an operator binds: {}",
        connector.base_url
    );
    assert!(
        !connector.base_url.contains('*'),
        "a wildcard in the base URL would widen the derived egress allow-list to every host: {}",
        connector.base_url
    );

    for (id, text) in emitted(&connector) {
        assert!(
            text.contains(r#"base = "https://{site}.atlassian.net/wiki""#),
            "`{id}` does not carry the unbound tenant template:\n{text}"
        );
    }
}

/// **Confluence asks for all three halves of its installation**: the site, and both halves of the
/// Basic credential.
///
/// This test used to be `no_secret_config_field_carries_an_example`, and it also re-checked that
/// every field was renderable. Both of those are catalogue-wide rules, and both are now **loader
/// refusals** in `connector_spec::provider` (C-231), asserted over the corpus by
/// `no_shipped_provider_gives_a_secret_field_an_example` — which reaches this file the way it
/// reaches every other provider, by reading `providers/` from disk. Restating a catalogue rule in
/// one connector's contract test is the defect C-230 is about, so what is left here is the part
/// that is genuinely about *this* connector: which questions it asks.
#[test]
fn the_configuration_surface_asks_for_the_site_and_both_credential_halves() {
    let connector = load();

    let asked: Vec<(&str, &str)> = connector
        .config
        .iter()
        .map(|field| (field.name.as_str(), field.binds.as_str()))
        .collect();
    assert_eq!(
        asked,
        vec![
            ("site", "endpoint.site"),
            ("email", "username.confluence.api_token"),
            ("api_token", "credential.confluence.api_token"),
        ],
        "a Confluence installation is a site plus a Basic pair; dropping one asks for a connector \
         that cannot compose a URL or authenticate"
    );
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and **loads** as exactly one exposed composite op.
#[test]
fn every_confluence_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
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
