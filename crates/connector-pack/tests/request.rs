//! **The request**, against real shipped operations.
//!
//! Every assertion here is on the request *before* it is sent, which is deliberate and is the
//! opposite of a compromise. The two mistakes this story can make are both ones a vendor answers
//! `200` to and then ignores: a body flattened out of the wire nesting the emitter honours
//! (`{"ticket.comment.body": …}` instead of `{"ticket": {"comment": {"body": …}}}`), and a query
//! string assembled without its `?`/`&` separators. A live call would prove neither, and a green
//! integration suite against a real vendor is exactly how both ship.
//!
//! **These requests are the unauthenticated ones**, and that is still true after C-116. Every
//! assertion below goes through `Operation::build_request` or [`Rehearsal::request`], which apply no
//! credential — it is the request the operation's own emitted module describes and nothing more.
//! `build_authenticated_request` is the one that resolves and places a credential, and
//! `tests/credentials.rs` is where it is followed. Keeping the two apart is what lets this file
//! assert a header set *exactly*.
//!
//! # The configuration is **declared**, never discovered (C-232)
//!
//! This file used to build its configuration port by asking each operation what variables it
//! *needed* and then manufacturing a value for every one of them. That made the test's input a
//! function of the very scan the test exists to check: whatever the pack decided it wanted, the
//! test supplied, so a value could never be missing and an operation that refuses in production
//! could never fail here. C-110 shipped eight operations that refused every call against an empty
//! configuration while `cargo test --workspace` was fully green, and this is the helper that hid it.
//!
//! What replaces it reads the connector's own `[[config]]` declarations out of
//! `providers/<id>.toml` — the field's `binds` target for the name, its `example` for the value —
//! and binds those and nothing else. A connector that declares no configuration is therefore
//! exercised against an **empty** configuration, which is its production shape and the case that was
//! never run.
//!
//! # And it is driven from disk, which is what makes it reach a provider story (C-233)
//!
//! The loop below enumerates `connectors/*.connector.toml` and reads each operation's Flux out of
//! `crates/catalog/ops/`. Both are **per-provider** artifacts a scoped
//! `flux-connectors build --provider <id>` writes, so an implementor whose connector is not yet in
//! the coordinator-owned index is covered by this test without regenerating anything and without
//! writing a test of their own. `tests/rehearsal.rs` is the same capability asked one operation at a
//! time.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{
    Configuration, Credentials, Egress, MemoryConfig, MemoryStore, Operation, Rehearsal, Request,
    DEFAULT_USER_AGENT,
};
use serde_json::{json, Value};

/// A stand-in for flux's `http.request`. Nothing here reaches it — `execute` needs a real
/// `ToolContext`, which needs a `flux_system::System` over a real workspace root — but a projected
/// operation needs *a* transport, and taking one is the seam the story is about.
fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        |params| async move { Ok(params) },
    ))
}

/// A bound credential port over an **empty** store (C-116).
///
/// The pack requires one; this file asserts the unauthenticated request, so it must hold nothing.
/// An empty store here is what keeps the header assertions below a statement about the *emitter*
/// rather than about whichever credential happened to resolve.
fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id")
}

/// The tenant both ports answer for — one constant, because a pack whose two ports name different
/// tenants is refused at install ([`connector_pack::Error::TenantMismatch`]).
const TENANT: &str = "t-request";

// ---------------------------------------------------------------------------------------------
// What the provider **declares**
// ---------------------------------------------------------------------------------------------

/// One `[[config]]` field, as its provider file declares it.
///
/// Four keys out of the block, because four are what a request needs: which variable the field
/// binds, which service it binds it under, and a value an operator would plausibly type. The rest —
/// `label`, `help`, `format`, `docs_url` — is renderer copy.
#[derive(Debug, Clone)]
struct Declared {
    /// The field's own name, unique across the connector. Used for diagnostics only: the *variable*
    /// is the `binds` suffix, and contentful declares `delivery_space_id` binding `space_id`.
    field: String,
    /// The service this field configures, or `None` for a field that applies to every service the
    /// connector has.
    service: Option<String>,
    /// The `binds` target, e.g. `endpoint.subdomain`, `path.zone_id`, `credential.zendesk.api_token`.
    binds: String,
    /// The further request positions the same value also reaches (C-229) — `providers/algolia.toml`
    /// is the shipped case, where one application id composes the host *and* travels as a header.
    also_binds: Vec<String>,
    /// The declared placeholder value, when the field declares one. Several deliberately do not —
    /// a realistic-looking placeholder for an opaque id reads as a real one belonging to a real
    /// organisation, and for a secret it has already tripped GitHub's push protection.
    example: Option<String>,
}

impl Declared {
    /// The configuration *variable* this field binds, when it binds one this pack resolves.
    ///
    /// The four binding kinds that reach a request: a templated base URL's `{variable}`
    /// (`endpoint.*`) and C-187's three pin positions. `credential.*` and `username.*` are the
    /// credential port's, not this one's.
    fn variable(&self) -> Option<&str> {
        let (kind, name) = self.binds.split_once('.')?;
        matches!(kind, "endpoint" | "path" | "query" | "header").then_some(name)
    }

    /// Every request header this field's value lands in — from `binds` or from `also_binds`.
    ///
    /// The one thing a configuration value may legitimately move besides a URL (C-187, C-229), and
    /// therefore the exemption the header assertion below is allowed to carry.
    fn pinned_headers(&self) -> Vec<&str> {
        std::iter::once(self.binds.as_str())
            .chain(self.also_binds.iter().map(String::as_str))
            .filter_map(|binds| binds.strip_prefix("header."))
            .collect()
    }

    /// The value an operator would supply, as the provider declares it.
    ///
    /// Falling back to a value derived from the field's own **name** keeps this a declaration: what
    /// is never taken from is the set of variables the pack's scan discovered, which is the
    /// dependency C-232 exists to break.
    fn value(&self) -> String {
        match &self.example {
            Some(example) => example.clone(),
            None => format!("a-{}", self.variable().unwrap_or(&self.field)),
        }
    }
}

/// One emitted service module, as `connectors/<id>[-<service>].connector.toml` describes it.
#[derive(Debug, Clone)]
struct Module {
    /// The connector id.
    connector: String,
    /// The service, `default` when the manifest elides it — the reserved name is never rendered.
    service: String,
    /// The service's own base URL, templating included.
    base_url: String,
    /// Every operation this service declares, in declaration order.
    operations: Vec<String>,
}

/// The repository root, from this crate's own manifest directory.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two levels above this crate")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// **A `key = "value"` line, or nothing.**
///
/// A deliberately tiny reader rather than a TOML parser, because `connector-pack` takes the
/// *catalogue* as its input and must not grow an edge to the loader — its manifest says so. It is
/// fail-closed instead of lenient: a value carrying a quote or a backslash panics rather than being
/// silently half-read, since a reader that quietly returns nothing is exactly the vacuous pass this
/// story is about. Every value it is asked for today is a bare identifier-ish string, and
/// [`the_declared_configuration_agrees_with_every_templated_base_url`] is the oracle that says so.
fn scalar(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| value_of(line, key))
}

fn value_of(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.strip_prefix(" = ")?;
    let value = rest.strip_prefix('"')?.strip_suffix('"')?;
    assert!(
        !value.contains(['"', '\\']),
        "`{key}` carries an escape this reader does not model: {line}"
    );
    Some(value.to_owned())
}

/// A `key = ["a", "b"]` line, as a list.
fn strings(text: &str, key: &str) -> Option<Vec<String>> {
    let line = text
        .lines()
        .find(|line| line.starts_with(key) && line[key.len()..].starts_with(" = ["))?;
    let inner = line
        .split_once('[')
        .and_then(|(_, rest)| rest.rsplit_once(']'))
        .map(|(inner, _)| inner)
        .expect("the line matched `key = [`, so it has both brackets");
    Some(
        inner
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| {
                item.strip_prefix('"')
                    .and_then(|item| item.strip_suffix('"'))
                    .unwrap_or_else(|| {
                        panic!("`{key}` holds something that is not a string: {item}")
                    })
                    .to_owned()
            })
            .collect(),
    )
}

/// Every emitted service module in the repository.
fn modules() -> Vec<Module> {
    let directory = root().join("connectors");
    let mut modules: Vec<Module> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.to_string_lossy().ends_with(".connector.toml"))
        .map(|path| {
            let text = read(&path);
            let named = |key: &str| {
                scalar(&text, key)
                    .unwrap_or_else(|| panic!("{} declares no `{key}`", path.display()))
            };
            Module {
                connector: named("connector"),
                // Elided for the reserved service, which is never rendered into a manifest, an
                // address or a file name.
                service: scalar(&text, "service").unwrap_or_else(|| "default".to_owned()),
                base_url: named("base_url"),
                operations: strings(&text, "operations")
                    .unwrap_or_else(|| panic!("{} declares no `operations`", path.display())),
            }
        })
        .collect();
    modules.sort_by(|left, right| {
        (&left.connector, &left.service).cmp(&(&right.connector, &right.service))
    });
    assert!(
        !modules.is_empty(),
        "no emitted service module was found under {}; an empty catalogue would pass every loop \
         in this file",
        directory.display()
    );
    modules
}

/// A `[[config]]` block being read, before its mandatory keys have been checked.
#[derive(Default)]
struct Block {
    name: Option<String>,
    service: Option<String>,
    binds: Option<String>,
    also_binds: Vec<String>,
    example: Option<String>,
}

impl Block {
    /// The block, or a panic naming what is missing. Fail-closed rather than skipped: a block this
    /// reader quietly dropped would put a variable back in the "nobody declares it, so nothing
    /// checks it" state the story is about.
    fn finish(self, path: &Path) -> Declared {
        let field = self
            .name
            .unwrap_or_else(|| panic!("a `[[config]]` block in {} has no `name`", path.display()));
        let binds = self
            .binds
            .unwrap_or_else(|| panic!("`{field}` in {} declares no `binds`", path.display()));
        Declared {
            field,
            service: self.service,
            binds,
            also_binds: self.also_binds,
            example: self.example,
        }
    }
}

/// Every `[[config]]` field `providers/<connector>.toml` declares.
///
/// A line reader over a block shape the JSON schema already constrains
/// (`schema/provider-toml.schema.json`), and fail-closed at every step: a block whose `name` or
/// `binds` is missing panics, and the number of blocks read is checked against the number of
/// `[[config]]` headers in the file. A reader that silently found nothing would restore exactly the
/// vacuous pass this story removes.
///
/// # `example` is the one key with no oracle, and that is a known gap
///
/// `the_declared_configuration_agrees_with_every_templated_base_url` cross-checks `name`, `service`
/// and `binds` against a *different* artifact — the emitted manifest's `base_url` — so dropping one
/// of those is loud. `example` has no second source: making this return `None` for every one of
/// them leaves both catalogue-wide tests green, because a synthesised `a-<variable>` fallback is a
/// perfectly good value for composing a request. It is caught only by the four hand-written URL
/// assertions above, which happen to name zendesk and freshdesk. The exposure is bounded — a
/// silently unused `example` weakens *how realistic* the values are, never whether a variable is
/// declared, which is the property C-232 is about — and it closes with C-87, when the configuration
/// surface reaches the catalogue and this reader is deleted.
fn declared_config(connector: &str) -> Vec<Declared> {
    let path = root().join("providers").join(format!("{connector}.toml"));
    let text = read(&path);

    let mut blocks: Vec<Declared> = Vec::new();
    let mut open: Option<Block> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            blocks.extend(open.take().map(|block| block.finish(&path)));
            if trimmed == "[[config]]" {
                open = Some(Block::default());
            }
            continue;
        }
        if let Some(block) = open.as_mut() {
            if let Some(value) = value_of(line, "name") {
                block.name = Some(value);
            } else if let Some(value) = value_of(line, "service") {
                block.service = Some(value);
            } else if let Some(value) = value_of(line, "binds") {
                block.binds = Some(value);
            } else if let Some(values) = strings(line, "also_binds") {
                block.also_binds = values;
            } else if let Some(value) = value_of(line, "example") {
                block.example = Some(value);
            }
        }
    }
    blocks.extend(open.take().map(|block| block.finish(&path)));

    let headers = text
        .lines()
        .filter(|line| line.trim() == "[[config]]")
        .count();
    assert_eq!(
        blocks.len(),
        headers,
        "{} declares {headers} `[[config]]` blocks and this reader found {}",
        path.display(),
        blocks.len()
    );
    blocks
}

/// The configuration variables this module's service declares, and the values declared for them.
fn declared_for(module: &Module) -> BTreeMap<String, String> {
    declared_config(&module.connector)
        .into_iter()
        .filter(|field| {
            field
                .service
                .as_ref()
                .is_none_or(|service| *service == module.service)
        })
        .filter_map(|field| {
            field
                .variable()
                .map(|variable| (variable.to_owned(), field.value()))
        })
        .collect()
}

/// The request headers this module's `[[config]]` fields pin, folded to lowercase.
///
/// Empty for every connector but one: `providers/algolia.toml` pins
/// `X-Algolia-Application-Id` from the same field that composes its host (C-229). This is the
/// exemption the header assertion carries, and it is derived from the provider file rather than
/// listed, so the next connector that pins a header is covered without anyone remembering to.
fn pinned_headers_for(module: &Module) -> BTreeSet<String> {
    declared_config(&module.connector)
        .into_iter()
        .filter(|field| {
            field
                .service
                .as_ref()
                .is_none_or(|service| *service == module.service)
        })
        .flat_map(|field| {
            field
                .pinned_headers()
                .into_iter()
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// **A bound configuration port carrying exactly what the shipped providers declare** — and no
/// value for anything they do not.
///
/// The URLs asserted below read `https://acme.zendesk.com/…` because `acme` is what
/// `providers/zendesk.toml` declares as its `subdomain` field's `example`. Nothing here is invented,
/// and nothing here is derived from what the pack asked for.
fn configuration() -> Configuration {
    let mut values = MemoryConfig::new();
    for module in modules() {
        for (variable, value) in declared_for(&module) {
            // Under the module's own service (C-197) — see `network_gate.rs` for why binding it
            // once per connector is not the same thing.
            values = values.with_endpoint(
                TENANT,
                &module.connector,
                &module.service,
                &variable,
                &value,
            );
        }
    }
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

/// The same port, over a **different** set of declared values.
///
/// Used to assert that a request body does not move when the configuration does. Prefixing a letter
/// keeps every value inside its own position's character set — a host label, a path segment, a query
/// value are all still what they were — so the second build differs from the first in the values
/// alone.
fn alternative_configuration() -> Configuration {
    let mut values = MemoryConfig::new();
    for module in modules() {
        for (variable, value) in declared_for(&module) {
            values = values.with_endpoint(
                TENANT,
                &module.connector,
                &module.service,
                &variable,
                &format!("x{value}"),
            );
        }
    }
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id")
}

/// An empty configuration port. Projection reads no *values* — it only reads the variables an
/// operation's own Flux names — so this is enough to ask an entry what it needs.
fn unconfigured() -> Configuration {
    Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant id")
}

fn project_with(entry: &'static catalog::Operation, configuration: Configuration) -> Operation {
    Operation::project(entry, http(), credentials(), configuration)
        .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id))
}

/// One shipped operation, projected.
fn projected(id: &str) -> Operation {
    let entry = catalog::operation(OperationKey::id(id))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
    project_with(entry, configuration())
}

/// The request `id` makes when called with `params`.
fn request(id: &str, params: Value) -> Request {
    projected(id)
        .build_request(&params)
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

/// **A nested body nests.** `zendesk-ticket-comment-add` writes `ticket.comment.body`, and the flat
/// spelling is a request Zendesk accepts and silently ignores — the comment never appears. The
/// emitter assembles the wire paths into one record ([`connector_flux`]'s `body_tree`); the pack
/// must land the caller's values at the same paths, or the two surfaces of one operation make two
/// different calls.
#[test]
fn a_nested_body_operation_nests_rather_than_flattening() {
    let request = request(
        "zendesk-ticket-comment-add",
        json!({
            "ticket_id": 42,
            "updated_stamp": "2026-07-30T00:00:00Z",
            "body": "the comment text",
            "public": false,
        }),
    );

    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.url,
        "https://acme.zendesk.com/api/v2/tickets/42.json"
    );

    let body: Value = serde_json::from_str(
        request
            .body
            .as_deref()
            .expect("a comment travels in a request body"),
    )
    .expect("the body is the JSON text `http.request` sends");

    assert_eq!(
        body,
        json!({
            "ticket": {
                "comment": {"body": "the comment text", "public": false},
                "safe_update": true,
                "updated_stamp": "2026-07-30T00:00:00Z",
            }
        }),
        "the wire paths must nest, and `ticket.safe_update` must be sent without being asked for"
    );

    // Not `{"ticket.comment.body": …}` — stated separately because it is the shape that would pass
    // a "the body mentions the text" assertion while being the wrong request.
    assert!(
        !request.body.as_deref().unwrap().contains("ticket.comment"),
        "a flattened dotted key is a request Zendesk accepts and ignores: {:?}",
        request.body
    );

    // Credentials are C-116. Asserting the whole header set rather than an absence keeps that
    // story's addition visible here — and C-223's, which is why `User-Agent` is spelled out rather
    // than filtered out: this connector declares none of its own, so the identity here is the
    // default, and a change to it must be seen in the one test that reads the whole set.
    assert_eq!(
        request.headers.iter().collect::<Vec<_>>(),
        vec![
            (&"User-Agent".to_string(), &DEFAULT_USER_AGENT.to_string()),
            (&"content-type".to_string(), &"application/json".to_string()),
        ]
    );
}

/// **A query string opens with `?` and continues with `&`.** `freshdesk-ticket-list` has four
/// optional filters and no required one, so the separator is carried by the emitted `$sep` symbol
/// and only the first *surviving* filter opens the query. Getting this wrong sends the vendor
/// `...tickets?requester_id=7?email=…`, which parses to one filter and drops the rest — answered
/// `200`, with a list that is simply not the list that was asked for.
#[test]
fn a_query_string_operation_separates_its_parameters() {
    // Every filter supplied: `?` then `&&&`.
    let all = request(
        "freshdesk-ticket-list",
        json!({
            "req_id": "7",
            "req_email": "a@b.c",
            "company_id": "9",
            "updated": "2026-07-30",
        }),
    );
    assert_eq!(
        all.url,
        "https://acme.freshdesk.com/api/v2/tickets?requester_id=7&email=a@b.c&company_id=9\
         &updated_since=2026-07-30"
    );
    assert_eq!(all.method, "GET");
    assert!(all.body.is_none(), "a listing sends no body");
    // No content type, because a `GET` sends no body — but not empty: since C-223 every request the
    // pack builds carries this software's identity, and a listing is the case that would otherwise
    // go out anonymous.
    assert_eq!(
        all.headers.iter().collect::<Vec<_>>(),
        vec![(&"User-Agent".to_string(), &DEFAULT_USER_AGENT.to_string())],
        "a listing sets no content type, and still identifies the software sending it"
    );

    // A middle filter only: it must be the one that opens the query with `?`, not the one that
    // inherits an `&` from a filter that was never sent.
    let one = request(
        "freshdesk-ticket-list",
        json!({
            "req_id": Value::Null,
            "req_email": Value::Null,
            "company_id": "9",
            "updated": Value::Null,
        }),
    );
    assert_eq!(
        one.url,
        "https://acme.freshdesk.com/api/v2/tickets?company_id=9"
    );

    // No filter at all: no `?`, and no dangling separator.
    let none = request(
        "freshdesk-ticket-list",
        json!({
            "req_id": Value::Null,
            "req_email": Value::Null,
            "company_id": Value::Null,
            "updated": Value::Null,
        }),
    );
    assert_eq!(none.url, "https://acme.freshdesk.com/api/v2/tickets");
}

/// A required query parameter goes in the template and an optional one is guarded, so the two kinds
/// have to agree about who owns the `?`. `zendesk-ticket-search` is the shipped case with both.
#[test]
fn a_required_query_parameter_opens_the_string_and_optional_ones_follow() {
    let both = request(
        "zendesk-ticket-search",
        json!({"query": "type:ticket status:new", "page": 2, "per_page": 50}),
    );
    assert_eq!(
        both.url,
        "https://acme.zendesk.com/api/v2/search.json\
         ?query=type:ticket status:new&page=2&per_page=50"
    );

    let required_only = request(
        "zendesk-ticket-search",
        json!({"query": "type:ticket", "page": Value::Null, "per_page": Value::Null}),
    );
    assert_eq!(
        required_only.url,
        "https://acme.zendesk.com/api/v2/search.json?query=type:ticket"
    );
}

/// A free-form body reaches the vendor whole, whether the caller spells it as a record or as JSON
/// text. Both spellings are why the emitter re-binds it through `parse(…, as: "json")` rather than
/// passing it straight to `http.request`.
///
/// **`babelforce-session-update`, not `babelforce-call-session-set`** (C-421). This test is about the
/// *emitter's* treatment of an opaque body, and it needs an operation whose body is opaque for
/// reasons that will not move. `babelforce-call-session-set`'s is not: the vendored 2026-07-10
/// document declares `SetCallSessionVariablesRequest` as a wrapper with one `variables` property,
/// where the older 0.7.0 reading the connector was hand-authored from had no `properties` at all —
/// so which body that operation sends is an open question about babelforce's wire contract (C-416
/// Progress §(a)), and this assertion has no stake in the answer. `babelforce-session-update`'s body
/// *is* the variable map, in both readings and both front-ends.
#[test]
fn a_free_form_body_travels_whole_in_either_spelling() {
    let as_record = request(
        "babelforce-session-update",
        json!({"id": "s-1", "body": {"appFoo": "bar"}}),
    );
    let as_text = request(
        "babelforce-session-update",
        json!({"id": "s-1", "body": "{\"appFoo\": \"bar\"}"}),
    );

    assert_eq!(as_record.body.as_deref(), Some(r#"{"appFoo":"bar"}"#));
    assert_eq!(as_record.body, as_text.body);
    assert_eq!(
        as_record.url,
        "https://services.babelforce.com/api/v2/sessions/s-1"
    );
}

/// The params handed to `http.request` are the shape its own input schema declares — `url` and
/// `method` always, `body` only when there is one.
///
/// **`headers` used to be conditional too, and since C-223 it is not.** The pack authors a
/// `User-Agent` on every request it builds, so the header record is never empty and `to_params`
/// always carries one. That is a deliberate consequence rather than an incidental one: the branch in
/// `to_params` that omits an empty record still exists and is still correct, and nothing in the
/// shipped catalogue can now reach it.
#[test]
fn the_request_becomes_the_params_http_request_declares() {
    let show = request("zendesk-ticket-show", json!({"ticket_id": 7}));
    assert_eq!(
        show.to_params(),
        json!({
            "url": "https://acme.zendesk.com/api/v2/tickets/7.json",
            "method": "GET",
            "headers": {"User-Agent": DEFAULT_USER_AGENT},
        })
    );

    let add = request(
        "zendesk-ticket-comment-add",
        json!({"ticket_id": 7, "updated_stamp": "s", "body": "b", "public": true}),
    );
    let params = add.to_params();
    assert_eq!(
        params["headers"],
        json!({
            "content-type": "application/json",
            "User-Agent": DEFAULT_USER_AGENT,
        })
    );
    assert!(
        params["body"].is_string(),
        "`http.request` reads `body` with `Value::as_str`, so an object would be dropped without a \
         word: {params}"
    );
}

/// **Every operation the repository declares composes a request from the configuration its provider
/// file declares** (C-232, C-233).
///
/// The replacement for `every_shipped_operation_builds_an_absolute_request`, and a replacement
/// rather than a second test beside it: two whole-catalogue request tests where one lies is worse
/// than one that is honest. Four things changed, and each one is a hole the old shape had.
///
/// 1. **The input is declared, not discovered.** The configuration port carries the `[[config]]`
///    fields the provider file declares and nothing else, so an operation needing a value nobody
///    declares refuses here — instead of being handed one manufactured out of whatever the pack's
///    scan happened to find.
/// 2. **A connector declaring no configuration is run against an empty one.** That is its production
///    shape, and it is the case that was never run: C-110's eight operations refused against exactly
///    this and `cargo test --workspace` stayed green.
/// 3. **The assertion is not only the URL.** A request whose URL composes while its body has been
///    rewritten by configuration substitution is the second half of what C-110's review found, so
///    every operation is built twice against two different declared configurations and the **body
///    must not move**, and no header moves except one the provider file *declares* as a pin. Nothing
///    today binds configuration into a body; the day something does, this fails and someone decides
///    it on purpose. The header half was a blanket refusal until C-229 shipped `providers/
///    algolia.toml`, whose application id composes the host and travels as `X-Algolia-Application-Id`
///    on every call — so it is now "only where declared", derived from the provider file, and the
///    pinned header is asserted to move rather than merely permitted to.
/// 4. **It is driven from per-provider artifacts**, so a provider story's own connector is covered
///    before it reaches the index.
#[test]
fn every_declared_operation_composes_a_request_from_its_declared_configuration() {
    let configuration = configuration();
    let alternative = alternative_configuration();
    let ops = root().join("crates/catalog/ops");

    let mut built = 0usize;
    let mut unconfigured_modules = 0usize;

    for module in modules() {
        let declared = declared_for(&module);
        if declared.is_empty() {
            unconfigured_modules += 1;
        }
        let pinned = pinned_headers_for(&module);
        let host = resolved_authority(&module.base_url, &declared);

        for id in &module.operations {
            let flux = read(&ops.join(&module.connector).join(format!("{id}.flux")));
            let rehearsal = Rehearsal::of(id, &module.connector, &module.service, &flux)
                .unwrap_or_else(|error| panic!("`{id}`: {error}"));

            // The crisp diagnostic, ahead of the build: a variable no `[[config]]` field declares is
            // a value no operator can ever supply, whatever the port happens to answer.
            let undeclared: Vec<&String> = rehearsal
                .endpoint_variables()
                .iter()
                .filter(|variable| !declared.contains_key(*variable))
                .collect();
            assert!(
                undeclared.is_empty(),
                "`{id}` needs {undeclared:?}, which `providers/{}.toml` declares no `[[config]]` \
                 field for — so no operator can supply it and the operation cannot be called",
                module.connector
            );

            let params = params_from_schema(&rehearsal);
            let request = rehearsal
                .request(&configuration, &params)
                .unwrap_or_else(|error| panic!("`{id}`: {error}"));

            assert!(
                request.url.starts_with("https://"),
                "`{id}` builds `{}`, which is not an absolute https URL",
                request.url
            );
            // **C-193, over the whole catalogue.** A brace surviving into a finished URL is a
            // request to a host that does not resolve — or, worse for the templated connectors, one
            // that resolves somewhere unintended.
            assert!(
                !request.url.contains('{'),
                "`{id}` builds `{}`, which still carries an unfilled configuration placeholder",
                request.url
            );
            assert!(
                request.url.contains(&host),
                "`{id}` builds `{}`, which does not reach its declared `{host}`",
                request.url
            );
            assert!(!request.method.is_empty(), "`{id}` builds no method");

            // **The half a URL-only assertion cannot see.** Same operation, same parameters,
            // different declared values: only the URL may move.
            let moved = rehearsal
                .request(&alternative, &params)
                .unwrap_or_else(|error| panic!("`{id}` (alternative configuration): {error}"));
            assert_eq!(
                request.body, moved.body,
                "`{id}`'s request body changed when its configuration did, so a tenant's settings \
                 are being substituted into something that is not a URL — the C-110 shape"
            );
            // **A header may move, and only where the provider file says it may.** C-187 made a
            // `[[config]]` value reach a request header on purpose and C-229 shipped the first
            // connector that uses it, so a blanket "headers never move" would now be asserting the
            // opposite of a declared feature. What is still asserted is the property that mattered:
            // every *other* header is untouched, so a tenant's settings are not leaking into a
            // header nobody declared — and the pinned one moved, which is what it is for.
            let expected: BTreeMap<&String, &String> = moved
                .headers
                .iter()
                .filter(|(header, _)| !pinned.contains(&header.to_ascii_lowercase()))
                .collect();
            let actual: BTreeMap<&String, &String> = request
                .headers
                .iter()
                .filter(|(header, _)| !pinned.contains(&header.to_ascii_lowercase()))
                .collect();
            assert_eq!(
                actual, expected,
                "`{id}`'s request headers changed when its configuration did, outside the headers \
                 `providers/{}.toml` pins ({pinned:?})",
                module.connector
            );
            for header in &pinned {
                let before = request
                    .headers
                    .iter()
                    .find(|(name, _)| name.to_ascii_lowercase() == *header);
                let after = moved
                    .headers
                    .iter()
                    .find(|(name, _)| name.to_ascii_lowercase() == *header);
                assert_ne!(
                    before, after,
                    "`{id}` pins {header:?} to a configuration value, but the header did not move \
                     when that value did — so the pin is not reaching the request"
                );
            }

            built += 1;
        }
    }

    assert!(built > 0, "an empty catalogue would pass the loop above");
    assert!(
        unconfigured_modules > 0,
        "no service module declares an empty configuration, so the production shape of an \
         unconfigured connector — the case C-110 was never run against — was not exercised"
    );
}

/// **The declared configuration agrees with what every templated base URL asks for.**
///
/// The oracle for the reader above. The loader already enforces that every `{variable}` in a base
/// URL is bound by exactly one `[[config]]` field and that a connector asks for nothing it cannot
/// use; asserting it here from two *different* artifacts — the emitted manifest and the provider
/// file — is what makes a mis-read block silent-proof. A reader that dropped a field would show up
/// as a service whose base URL names a variable nothing declares.
#[test]
fn the_declared_configuration_agrees_with_every_templated_base_url() {
    let mut templated = 0usize;
    for module in modules() {
        let declared = declared_for(&module);
        let endpoints: BTreeSet<String> = declared_config(&module.connector)
            .into_iter()
            .filter(|field| {
                field
                    .service
                    .as_ref()
                    .is_none_or(|service| *service == module.service)
            })
            .filter(|field| field.binds.starts_with("endpoint."))
            .filter_map(|field| field.variable().map(str::to_owned))
            .collect();

        let wanted = placeholders(&module.base_url);
        assert_eq!(
            wanted, endpoints,
            "`{}`/`{}` templates `{}` but declares `endpoint.*` fields for {endpoints:?}",
            module.connector, module.service, module.base_url
        );
        if !wanted.is_empty() {
            templated += 1;
            for variable in &wanted {
                assert!(
                    declared.contains_key(variable),
                    "`{}`/`{}` declares no value for `{variable}`",
                    module.connector,
                    module.service
                );
            }
        }
    }
    assert!(
        templated > 0,
        "no service module has a templated base URL, so this asserted nothing"
    );
}

/// Every `{name}` in `template`.
fn placeholders(template: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        found.insert(after[..close].trim().to_owned());
        rest = &after[close + 1..];
    }
    found
}

/// The authority a service's base URL resolves to once its declared configuration is substituted in.
///
/// `base_url` is a *template* for the templated connectors — `https://{subdomain}.zendesk.com`.
/// Comparing a built URL against the template would have been the assertion that quietly stopped
/// meaning anything the moment substitution started working.
fn resolved_authority(base_url: &str, declared: &BTreeMap<String, String>) -> String {
    let mut filled = base_url.to_owned();
    for (variable, value) in declared {
        filled = filled.replace(&format!("{{{variable}}}"), value);
    }
    let after_scheme = filled
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&filled);
    after_scheme
        .split(['/', '?', '#'])
        .next()
        .expect("a split always yields a first element")
        .to_owned()
}

/// A plausible value for every parameter an operation declares, from its own input schema.
fn params_from_schema(rehearsal: &Rehearsal) -> Value {
    let spec = rehearsal.spec();
    let mut params = serde_json::Map::new();
    if let Some(properties) = spec
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    {
        for (name, schema) in properties {
            let value = match schema.get("type").and_then(Value::as_str) {
                Some("number") | Some("integer") => json!(1),
                Some("boolean") => json!(true),
                Some("array") => json!([]),
                Some("object") => json!({}),
                Some(_) => Value::String(format!("a-{name}")),
                // An untyped schema is a free-form body (`Any`), which travels through
                // `parse(…, as: "json")` — a bare string is not JSON and would be refused.
                None => json!({}),
            };
            params.insert(name.clone(), value);
        }
    }
    Value::Object(params)
}

/// The projected path and the rehearsed path must answer the same thing for a shipped operation.
///
/// Without this the loop above could drift into testing a second implementation of "build a
/// request": [`Rehearsal`] exists so a connector that is *not* in the index can be exercised, and it
/// is worth nothing if it is not the same code path a host runs.
#[test]
fn a_rehearsal_and_a_projection_agree_on_a_shipped_operation() {
    let entry = catalog::operation(OperationKey::id("zendesk-ticket-show"))
        .expect("the shipped catalogue carries zendesk-ticket-show");
    let rehearsal = Rehearsal::of(entry.id, entry.provider, entry.service, entry.flux)
        .expect("a shipped operation rehearses");

    let params = json!({"ticket_id": 7});
    assert_eq!(
        rehearsal
            .request(&configuration(), &params)
            .expect("the rehearsed request composes")
            .to_params(),
        project_with(entry, configuration())
            .build_request(&params)
            .expect("the projected request composes")
            .to_params()
    );
}

/// **The refusal runs at the projection call site too, and this is what pins it** (C-232).
///
/// `refuse_unconfigurable` is called twice — in `Operation::project` and again in `request::build` —
/// on the stated ground that "the answer must not depend on which entry point a host reaches".
/// Without this test nothing executed the first of the two: deleting the call in
/// `crates/connector-pack/src/tool.rs` left the whole workspace green, so a guarantee the doc
/// asserted was carried by the *other* call site alone. A doubled check that only one half of is
/// pinned is a single check with a comment claiming otherwise.
///
/// # Why the entry has to be doctored, and why that is not the C-233 route
///
/// `catalog::Operation` is `#[non_exhaustive]`, so no synthetic one can be *constructed* here. A
/// shipped one can be **copied** and its `pub` fields overwritten, which is what
/// `tests/differential.rs` does to compare two artifacts — and it is enough to reach
/// `Operation::project` with a body that must be refused.
///
/// It is not, however, a route a provider implementor could have used, which is why
/// [`connector_pack::Rehearsal`] still exists. Doctoring gives you *another connector's* entry
/// wearing your Flux: the id, the service, the declared hosts and the credentials are still the
/// shipped one's, `project` refuses a declaration whose name disagrees with `entry.id`
/// (`Error::Mismatched`), and correcting `provider` to your own then fails the index lookup with
/// `Error::UnknownProvider`. What it can do is exactly what it does here — take one *shipped*
/// operation and give it a different body.
#[test]
fn a_document_literal_is_refused_at_projection_and_not_only_at_build() {
    const OPERATION: &str = "zendesk-ticket-show";

    /// The shipped operation's own id and metadata, with C-110's body. Deliberately still a
    /// templated base URL, so the only thing this can be refused for is the document.
    const DOCTORED: &str = r#"op zendesk-ticket-show -> Any
  description "Show one ticket"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """query Viewer {
  viewer {
    displayName
  }
}
"""
  payload = { query }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
"#;

    let entry = catalog::operation(OperationKey::id(OPERATION))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{OPERATION}`"));

    // The control: the real entry projects, so a failure below is the doctoring and not the fixture.
    Operation::project(entry, http(), credentials(), configuration())
        .expect("the shipped entry projects");

    let mut doctored = *entry;
    doctored.flux = DOCTORED;
    let doctored: &'static catalog::Operation = Box::leak(Box::new(doctored));

    let error = Operation::project(doctored, http(), credentials(), configuration())
        .expect_err("a body binding an unclassifiable literal must not install");
    assert!(
        matches!(error, connector_pack::Error::Unbuildable { .. }),
        "refused at projection, but for the wrong reason: {error}"
    );
    assert!(
        error.to_string().contains("displayName"),
        "the refusal must quote the literal it could not classify: {error}"
    );
}

/// **Every operation this repository declares carries exactly one `User-Agent`** (C-223).
///
/// A property over whatever ships rather than a census of what ships today, which is the shape
/// `AGENTS.md` requires of a catalogue-wide claim: a forty-sixth connector satisfying it leaves this
/// green, and one violating it is exactly when this should fail.
///
/// **Exactly one** is the load-bearing half, and it is why the count is taken case-insensitively
/// over the header *names*. `Request::headers` is a `BTreeMap`, so a connector declaring
/// `user-agent` while the default inserts `User-Agent` would produce two entries, two JSON keys in
/// `to_params`, and either two headers on the wire or a silent overwrite depending on how the
/// transport folds them. A duplicated `User-Agent` is its own defect, and asserting presence alone
/// would report it as success.
///
/// Driven from the same per-provider artifacts as
/// [`every_declared_operation_composes_a_request_from_its_declared_configuration`], so a provider
/// story's own connector is covered before it reaches the index.
#[test]
fn every_declared_operation_carries_exactly_one_user_agent() {
    let configuration = configuration();
    let ops = root().join("crates/catalog/ops");
    let mut checked = 0usize;

    for module in modules() {
        for id in &module.operations {
            let flux = read(&ops.join(&module.connector).join(format!("{id}.flux")));
            let rehearsal = Rehearsal::of(id, &module.connector, &module.service, &flux)
                .unwrap_or_else(|error| panic!("`{id}` does not rehearse: {error}"));
            let Ok(request) = rehearsal.request(&configuration, &params_from_schema(&rehearsal))
            else {
                // Composability is `every_declared_operation_composes_a_request_from_its_declared_
                // configuration`'s assertion, and duplicating it here would report one defect twice.
                continue;
            };

            let agents: Vec<&String> = request
                .headers
                .keys()
                .filter(|name| name.eq_ignore_ascii_case("User-Agent"))
                .collect();
            assert_eq!(
                agents.len(),
                1,
                "`{id}` carries {} `User-Agent` headers, not one: {:?}",
                agents.len(),
                request.headers.keys().collect::<Vec<_>>()
            );
            let value = &request.headers[agents[0]];
            assert!(
                !value.trim().is_empty(),
                "`{id}` carries an empty `User-Agent`, which a vendor reads as absent"
            );
            checked += 1;
        }
    }

    assert!(
        checked > 100,
        "only {checked} operations were checked, so this property was quantified over almost \
         nothing"
    );
}

/// The Resend operations the premise below is about, **named rather than discovered**. This is a
/// claim about specific connectors, so `AGENTS.md` keeps it to a closed set: only these connectors
/// changing can falsify it, and a forty-sixth connector cannot.
const RESEND_OPERATIONS: &[&str] = &[
    "resend-email-send",
    "resend-email-get",
    "resend-domain-list",
    "resend-domain-get",
];

/// Resend declares no service, so its operations belong to the reserved `default`.
const RESEND_SERVICE: &str = "default";

/// **Resend inherits the versioned host identity rather than overriding it with a bare word**
/// (C-241).
///
/// C-223 gave every connector
/// [`DEFAULT_USER_AGENT`] and let a connector's own declaration win where it had one. Exactly four
/// operations were unchanged by that — Resend's, because `providers/resend.toml` declared
/// `const_headers = { "User-Agent" = "flux-connectors" }`. The declaration was correct when it was
/// written and is now the *worse* of the two values available: it carries no version, so a vendor
/// cannot tell one release from another, and a bare product word is what C-223's own acceptance
/// rules out.
///
/// **The vendor requirement it was written for has not gone away** — Resend answers a request with
/// no `User-Agent` with a `403` carrying a valid key — which is why this test asserts the header is
/// *present and versioned* rather than merely that the declaration is gone. Removing a workaround
/// and leaving the operation bare would satisfy the second half of that sentence and re-open the
/// `403`.
///
/// Driven from the per-provider artifacts `build --provider resend` writes, like
/// [`every_declared_operation_carries_exactly_one_user_agent`], so the two agree on where they read
/// the connector from.
#[test]
fn resend_inherits_the_versioned_host_identity() {
    let configuration = configuration();
    let ops = root().join("crates/catalog/ops/resend");

    for id in RESEND_OPERATIONS {
        let flux = read(&ops.join(format!("{id}.flux")));
        let rehearsal = Rehearsal::of(id, "resend", RESEND_SERVICE, &flux)
            .unwrap_or_else(|error| panic!("`{id}` does not rehearse: {error}"));
        let request = rehearsal
            .request(&configuration, &params_from_schema(&rehearsal))
            .unwrap_or_else(|error| panic!("`{id}` composes no request: {error}"));

        // Presence and uniqueness first: this is what proves removing the declaration did not leave
        // the operation with no identity at all, which is the failure Resend answers with a `403`.
        let agents: Vec<&String> = request
            .headers
            .keys()
            .filter(|name| name.eq_ignore_ascii_case("User-Agent"))
            .collect();
        assert_eq!(
            agents,
            vec!["User-Agent"],
            "`{id}` must carry exactly one `User-Agent`, spelled canonically: {:?}",
            request.headers.keys().collect::<Vec<_>>()
        );

        let value = &request.headers[agents[0]];
        assert_eq!(
            value, DEFAULT_USER_AGENT,
            "`{id}` sends `{value}` rather than the versioned host identity"
        );

        // The first product token, whole — the form the design record settles on, because a value
        // merely *containing* `flux-connectors` is satisfied by the repository URL in the trailing
        // comment. A bare `flux-connectors` has no `/`, so this is exactly the assertion the shipped
        // declaration fails.
        let product = value
            .split_whitespace()
            .next()
            .expect("a `User-Agent` value is not empty");
        let (name, version) = product.split_once('/').unwrap_or_else(|| {
            panic!(
                "`{id}` sends `{product}`, a product token with no version — a vendor cannot tell \
                 one release of this software from another"
            )
        });
        assert_eq!(name, "flux-connectors");
        assert!(
            !version.is_empty(),
            "`{id}` sends an empty version in `{product}`"
        );
    }
}

/// **A connector that declares its own `User-Agent` keeps it, and gains no second one** (C-223).
///
/// **Both halves are fixtures now, and that is a change of evidence rather than of claim** (C-241).
/// The first half used to be asserted against `providers/resend.toml`, the one shipped connector
/// that declared the header. C-241 removed that declaration — it had become a bare, versionless
/// override of the host identity — and *no* connector declares one today. So there is no shipped
/// case left to point at, and pinning this behaviour to whichever connector happens to declare one
/// next would make the test's premise a coincidence.
///
/// What is doctored is a real emitted module rather than a hand-written string: `github-issue-get`
/// ships a constant `Accept` header, which is the exact shape the emitter gives any constant header,
/// and it is rewritten here into the two spellings that matter.
///
/// The `user-agent` half is the defect-prevention one and is invisible until some connector spells
/// it that way. `Request::headers` is a `BTreeMap`, so a lowercase declaration against a
/// `User-Agent` default is two entries, two JSON keys in `to_params`, and either two headers on the
/// wire or a silent overwrite. `identify`'s guard is case-insensitive for exactly this, and this is
/// where that is proved rather than asserted in a comment.
#[test]
fn a_connector_declaring_its_own_user_agent_wins_and_gains_no_second_one() {
    /// A value that is plainly neither the host default nor a truncation of it, so a test failure
    /// cannot be read as "the two happen to agree".
    const DECLARED: &str = "vendor-pinned-agent/9.9";

    let entry = catalog::operation(OperationKey::id("github-issue-get"))
        .expect("the shipped catalogue carries github-issue-get");

    // `Accept = "…"` bound to a symbol and named in the request's header record is what the emitter
    // writes for every constant header, so rewriting it in place yields a module in exactly the
    // shape a connector declaring a `User-Agent` would ship.
    let declaring = |name: &str, symbol: &str| {
        let flux = entry
            .flux
            .replace(
                "Accept = \"application/vnd.github+json\"",
                &format!("{symbol} = \"{DECLARED}\""),
            )
            .replace(
                "headers: { Accept }",
                &format!("headers: {{ \"{name}\": {symbol} }}"),
            );
        assert!(
            flux.contains(&format!("\"{name}\": {symbol}")),
            "the doctoring did not apply, so this half proves nothing:\n{flux}"
        );
        let rehearsal = Rehearsal::of(entry.id, entry.provider, entry.service, &flux)
            .unwrap_or_else(|error| panic!("the doctored `{name}` declaration rehearses: {error}"));
        rehearsal
            .request(
                &configuration(),
                &json!({"owner": "codewandler", "repo": "flux-connectors", "issue_number": 1}),
            )
            .unwrap_or_else(|error| {
                panic!("the doctored `{name}` declaration builds its request: {error}")
            })
    };

    for (name, symbol) in [("User-Agent", "User_Agent"), ("user-agent", "user_agent")] {
        let request = declaring(name, symbol);

        let agents: Vec<&String> = request
            .headers
            .keys()
            .filter(|header| header.eq_ignore_ascii_case("User-Agent"))
            .collect();
        assert_eq!(
            agents,
            vec![name],
            "a declared `{name}` was joined by a second `User-Agent` rather than kept: {:?}",
            request.headers.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            request.headers[name], DECLARED,
            "a connector's declared `{name}` was overwritten by the host default"
        );
        assert_ne!(
            request.headers[name], DEFAULT_USER_AGENT,
            "the fixture no longer distinguishes the two, so this proves nothing"
        );
    }
}

/// Projection reads variables and not values, so an unconfigured port still answers what an
/// operation needs — the fact `configuration()` used to be built on, kept as an explicit statement
/// now that it is not.
#[test]
fn projection_needs_no_values_to_report_what_an_operation_needs() {
    let entry = catalog::operation(OperationKey::id("zendesk-ticket-show"))
        .expect("the shipped catalogue carries zendesk-ticket-show");
    assert_eq!(
        project_with(entry, unconfigured()).endpoint_variables(),
        ["subdomain"]
    );
}
