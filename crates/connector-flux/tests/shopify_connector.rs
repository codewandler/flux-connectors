//! `providers/shopify.toml` exists, emits analyzable Flux, and holds the four properties C-74 ships
//! it for. Three of them are safety claims and one is a versioning claim; none of them is style.
//!
//! **1. The credential is a plain custom header.** Shopify sends `X-Shopify-Access-Token: <token>`
//! with no scheme word in front of it, which is [`AuthScheme::Header`] exactly — the whole header
//! value *is* the secret. That makes this connector the cheapest end-to-end proof that the `Header`
//! variant round-trips through the loader, the emitter, the manifest and the catalogue, and it needs
//! nothing from the prefix axis C-19 has not built: there is no prefix to build.
//!
//! **2. No query parameter, of any type.** Nothing in this pipeline percent-encodes a query value —
//! the emitter interpolates it verbatim into a `fmt` template (`crates/connector-flux/src/op.rs`) and
//! flux registers no URL-encoding op, so C-30 is unimplemented and `zendesk-ticket-search` is the
//! standing demonstration AGENTS.md records under *Intentional gaps*. Shopify's collection endpoints
//! are exactly the shape that trips it (`?status=`, `?limit=`, `?ids=`), so they are excluded and the
//! absence is asserted in the strong form: **zero** query parameters, not merely zero string-ish
//! ones. Stated over the IR *and* over the emitted text, because the two can disagree.
//!
//! **3. No optional request-body field.** An omitted optional body field is emitted as an explicit
//! `null` (C-56), and Shopify's product update takes a partial `{"product": {…}}` object where the
//! *absence* of a key is what means "leave this alone". A `null` there is a request to clear the
//! field, not to skip it, so an optional field would turn one edit into a silent wipe. Until C-56
//! lands this connector declares required fields only.
//!
//! **4. The API version is in the path, and every path agrees.** Shopify spells its version into
//! every URL (`/admin/api/2024-10/…`), so the version is not connector metadata that happens to be
//! written down — it is part of five separate strings. [`the_api_version_is_one_value_across_every_path`]
//! is what makes them agree by construction today: one constant, checked against all five paths, so a
//! half-finished version bump fails rather than shipping a connector that speaks two versions. When
//! C-49's per-service `api_version` lands, the prefix becomes derived from that field and this test
//! becomes the check that the derivation actually happened.
//!
//! The structural claims deliberately restate what `shipped_modules.rs` asserts across every
//! provider, so C-74's gate fails on its own file rather than only inside a shared loop.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod};

/// The provider under test. Named once so the file reads as being about Shopify rather than about a
/// string.
const PROVIDER: &str = "shopify";

/// The credential the connector declares, the header it travels in, and the variable it resolves
/// from. All three are public contract — an operator sets the variable, a manifest names the
/// credential, Shopify reads the header — so they are pinned here rather than left to whatever the
/// provider file happens to say.
const CREDENTIAL: &str = "shopify.access_token";
/// See [`CREDENTIAL`]. The custom header Shopify authenticates on.
const AUTH_HEADER: &str = "X-Shopify-Access-Token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "SHOPIFY_ACCESS_TOKEN";

/// The Admin API version every path is pinned to. One value, five paths — see
/// [`the_api_version_is_one_value_across_every_path`].
const API_VERSION: &str = "2024-10";

/// The unbound tenant template in the base URL. Every Shopify store has its own `*.myshopify.com`
/// host, so there is no tenant-independent base URL, exactly as for zendesk's `{subdomain}`.
const TENANT_TEMPLATE: &str = "{shop}";

/// The base URL, tenant template included. C-68 owns binding it; nothing here invents a binding.
const BASE_URL: &str = "https://{shop}.myshopify.com";

/// The five curated operations, in the order `providers/shopify.toml` declares them.
const OPERATIONS: &[&str] = &[
    "shopify-order-get",
    "shopify-product-get",
    "shopify-product-update",
    "shopify-customer-get",
    "shopify-inventory-level-list",
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
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-74 ships the Shopify connector",
            path.display()
        )
    });
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Every operation's emitted module, paired with its id.
fn emitted() -> Vec<(String, String)> {
    let connector = load();
    connector
        .operations
        .iter()
        .map(|operation| {
            let flux = emit_operation(&connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            (operation.id.clone(), flux)
        })
        .collect()
}

/// **The `AuthScheme::Header` round trip.** The connector loads, and its credential is a custom
/// header carrying the whole secret with no prefix.
#[test]
fn the_shopify_connector_authenticates_with_a_plain_custom_header() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Shopify");
    assert_eq!(connector.base_url, BASE_URL);

    assert_eq!(
        connector.auth.len(),
        1,
        "shopify authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("shopify declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string(),
            prefix: String::new(),
        },
        "Shopify's token is the entire value of `{AUTH_HEADER}` — no `Bearer `, no `Basic `, no \
         scheme word of any kind — which is what `AuthScheme::Header` means and why this connector \
         needs nothing from C-19's prefix axis"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a custom header has no user half"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    // Every operation resolves to the one credential, whether it declares auth or inherits the
    // connector default.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; shopify is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the access token",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; `{AUTH_HEADER}` is injected by the \
             host and must never travel through the parameter surface, where a caller could set it \
             to anything",
            operation.id
        );
    }
}

/// **The API version is one value, and every path carries it.** See the module docs: the version is
/// not metadata here, it is a literal segment of five separate strings, and the failure mode a
/// hand-edited string per operation invites is a connector that speaks two versions at once.
///
/// The assertion is deliberately two-sided. Every path must start with the versioned prefix, *and*
/// no path may mention any other `/admin/api/<something>/` — so a bump that reached four of five
/// paths fails here rather than at a vendor.
#[test]
fn the_api_version_is_one_value_across_every_path() {
    let connector = load();
    let prefix = format!("/admin/api/{API_VERSION}/");

    for operation in &connector.operations {
        assert!(
            operation.path.starts_with(&prefix),
            "operation `{}` has path `{}`, which does not begin with `{prefix}`. Shopify spells the \
             API version into every URL, so the five paths have to agree; when C-49's per-service \
             `api_version` lands, this prefix is derived from that field rather than typed five \
             times",
            operation.id,
            operation.path
        );
        assert_eq!(
            operation.path.matches("/admin/api/").count(),
            1,
            "operation `{}` names `/admin/api/` more than once in `{}`",
            operation.id,
            operation.path
        );
    }

    // The emitted URL is what actually reaches Shopify, and it is assembled from the path template
    // plus `$base`. Checking it too is what stops a future emitter change from rewriting the prefix.
    for (id, flux) in emitted() {
        assert!(
            flux.contains(&prefix),
            "`{id}` emits no `{prefix}` in its request URL:\n{flux}"
        );
    }
}

/// **The `{shop}` tenant template is recorded, not bound.** Every Shopify store lives on its own
/// `*.myshopify.com` host, so there is no tenant-independent base URL — the same situation zendesk's
/// `{subdomain}` is in, and the same one C-68 owns.
///
/// Nothing here invents a binding. What is asserted is that the template survives into the emitted
/// module verbatim, which is what makes `status.rs` publish the `unbound-base-url-template` issue for
/// every Shopify operation instead of shipping a connector that looks addressable and is not.
#[test]
fn the_shop_tenant_template_reaches_the_module_unbound() {
    let connector = load();

    assert!(
        connector.base_url.contains(TENANT_TEMPLATE),
        "shopify's base URL `{}` names no `{TENANT_TEMPLATE}`; a store host is per-tenant and \
         cannot be a constant",
        connector.base_url
    );

    for (id, flux) in emitted() {
        assert!(
            flux.contains(&format!("base = \"{BASE_URL}\"")),
            "`{id}` does not bind `$base` to the tenant-templated base URL, so either the template \
             was resolved somewhere it should not have been or the URL changed:\n{flux}"
        );
    }
}

/// **No query parameter of any type.** See the module docs: nothing percent-encodes a query value, so
/// the honest form of "this connector has no encoding gap" is that its query surface is empty.
///
/// The strong form rather than a check on the parameter's *type*, because "string-ish" invites a later
/// author to add an `integer` filter and rebuild the query-assembly path this connector exists to
/// avoid.
#[test]
fn no_shopify_operation_declares_a_query_parameter() {
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
            "operation `{}` declares query parameters {declared:?}. Nothing percent-encodes a query \
             value (C-30 is unimplemented), so a value carrying `&` or `#` corrupts the request or \
             injects a parameter — the `zendesk-ticket-search` failure AGENTS.md records. C-74 ships \
             the path-and-body surface only; if C-30 has landed, change this test deliberately",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads — and the half that
/// takes care.
///
/// **Every `url = ` line is checked, not just the first.** The emitter binds `$url` once for the path
/// and the required query parameters, then re-binds it once more per *optional* query parameter inside
/// a `when` guard (`crates/connector-flux/src/op.rs`, the `optional` loop), and the `?` lives on a
/// separate `sep` binding rather than on the `$url` line. `connectors/zendesk.flux` shows the shape:
///
/// ```flux
/// url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
/// sep = "?"
/// when $page
///   url = fmt("{url}{sep}page={page}")
/// ```
///
/// So inspecting only the first binding, or only looking for a literal `?`, would pass while an
/// operation quietly appended optional filters. All three are checked: one `$url` binding, no `?`
/// anywhere, and no `sep` at all.
#[test]
fn no_shopify_operation_emits_a_query_string() {
    for (id, flux) in emitted() {
        let url_lines: Vec<&str> = flux
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        assert!(!url_lines.is_empty(), "`{id}` binds no $url:\n{flux}");
        assert_eq!(
            url_lines.len(),
            1,
            "`{id}` re-binds $url {} times; the emitter does that once per optional query \
             parameter, so this operation is appending a query string:\n{flux}",
            url_lines.len()
        );
        assert!(
            !flux.contains('?'),
            "`{id}` emits a `?`, so a value is reaching the query string unencoded:\n{flux}"
        );
        // `sep` exists only to carry the `?`/`&` between query parameters, so an operation that
        // binds it is building a query string even if no single line spells the `?`.
        assert!(
            !flux
                .lines()
                .any(|line| line.trim_start().starts_with("sep = ")),
            "`{id}` binds $sep, which the emitter emits only to separate query parameters:\n{flux}"
        );
    }
}

/// **No optional request-body field**, until C-56 lands.
///
/// An omitted optional field travels as an explicit `null`. On Shopify's product update the request
/// body is a *partial* `{"product": {…}}` object where an absent key means "leave this alone" and a
/// `null` means "clear it", so an optional `body_html` would let a caller who set only `title` wipe
/// the description. That is the silent-wrong-result class this repository refuses, so the whole
/// declared body surface is required.
#[test]
fn no_shopify_operation_declares_an_optional_body_field() {
    let connector = load();

    for operation in &connector.operations {
        let optional: Vec<&str> = operation
            .params
            .body
            .iter()
            .filter(|param| !param.required)
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            optional.is_empty(),
            "operation `{}` declares optional body fields {optional:?}. An omitted optional field \
             is emitted as an explicit `null` (C-56), and Shopify reads a `null` in a partial \
             update as \"clear this\" rather than \"skip this\"",
            operation.id
        );
    }
}

/// **The write is a write.** `risk` and `idempotency` are what flux's approval gate reads, so a
/// mutating method must not carry a read's metadata — and the one write here has no
/// optimistic-concurrency token to make a replay safe.
#[test]
fn the_product_update_is_the_only_write_and_declares_itself_as_one() {
    let connector = load();

    for operation in &connector.operations {
        let mutating = matches!(
            operation.method,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete
        );
        if !mutating {
            assert!(
                operation.params.body.is_empty(),
                "read `{}` declares body fields; a Shopify read takes its id in the path",
                operation.id
            );
            continue;
        }

        assert_eq!(
            operation.id, "shopify-product-update",
            "`{}` mutates the store, and C-74 curates exactly one write; a second needs its own \
             review of what it changes and who sees it",
            operation.id
        );
        assert_ne!(
            operation.risk,
            connector_spec::Risk::Low,
            "a write may not be `low`: a product edit is visible on the public storefront the \
             moment it saves"
        );
        assert_eq!(
            operation.idempotency,
            connector_spec::Idempotency::NonIdempotent,
            "Shopify's REST product update is last-write-wins — it publishes no idempotency key \
             and takes no `updated_at` precondition — so a replay overwrites whatever landed in \
             between rather than repeating one effect"
        );
    }
}

/// **No credential reaches a generated module** — not its value, and today not even its name.
///
/// Auth injection is C-10 and the `$auth` seam it needs must land in flux first, so the emitted `op`
/// builds a URL and calls `http.request` with `method` and `url` and nothing else. That is a recorded
/// gap rather than a bug, and this pins the direction of it: a future edit that starts splicing the
/// token into the module fails here.
///
/// The `http_hosts` half sits alongside it, because it is the same claim about the same text: the one
/// host the module names is the one the base URL derives, never a wildcard.
#[test]
fn no_credential_and_no_widened_host_reaches_a_generated_module() {
    for (id, flux) in emitted() {
        assert!(
            !flux.contains(TOKEN_ENV),
            "`{id}` names the credential's environment variable; a generated module carries no \
             credential at all until C-10:\n{flux}"
        );
        assert!(
            !flux.contains(AUTH_HEADER),
            "`{id}` spells the auth header into the request; the host applies the placement \
             scheme, not generated Flux:\n{flux}"
        );
        assert_eq!(
            flux.matches("https://").count(),
            1,
            "`{id}` names more than one absolute URL, so `http_hosts` would have to be widened \
             beyond what the base URL derives:\n{flux}"
        );
        assert!(
            flux.contains(&format!("base = \"{BASE_URL}\"")),
            "`{id}` does not reach `{BASE_URL}`:\n{flux}"
        );
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical
/// under flux's own formatter, and **loads** as exactly one exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set. It is restated here so C-74's own
/// test file fails on its own when the module stops being analyzable — a provider whose emitted Flux
/// does not load publishes no ops at all, and a consumer handing it to flux would get silence rather
/// than an error.
#[test]
fn every_shopify_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
    for (id, flux) in emitted() {
        let parsed = flux_lang::parser::parse_cst(&flux);
        assert!(
            parsed.errors.is_empty(),
            "`{id}` emits Flux that does not parse: {:?}\n{flux}",
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(flux.as_str()),
            "the flux formatter would rewrite `{id}`"
        );

        let module = flux_lang::program::Module::parse_str(&flux)
            .unwrap_or_else(|error| panic!("`{id}` does not load: {error}"));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{id}` is not a program"));
        assert_eq!(
            program.ops.len(),
            1,
            "one operation is one declaration; `{id}` loaded {}",
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, id);
        assert!(
            program.ops[0].meta.expose,
            "`{id}` must be exposed to the model as a tool"
        );
    }
}
