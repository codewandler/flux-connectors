//! **The first byte this repository sends, asserted against a vendor under test control.**
//!
//! Every other test in this crate stops at or before the socket, and `tests/host.rs` records why
//! that was the honest place to stop while nothing here could send at all. This file is the one
//! that sends: a real `flux_web::http::HttpRequestTool`, wrapped in the pack's own `Egress`,
//! carrying a shipped operation's request to a loopback HTTP server that records exactly what
//! arrived. The assertion is equality between the `{ method, url, headers, body }` the pack built
//! and the `{ method, url, headers, body }` the vendor received.
//!
//! # The tension this file had to resolve, and how
//!
//! The host configures `PrivateNetAllow::None` — the full SSRF guard — so [`App::new`]'s egress
//! **refuses loopback**, which is precisely where a vendor under test control has to live. The
//! guard that makes the host safe is the guard that blocks proving it can send. Two things were
//! weighed:
//!
//! - **Not the guard.** Nothing here relaxes `WebOptions::default()`, and nothing here asserts
//!   against a weakened default. [`App::with_web_options`] takes the policy as an argument and
//!   [`App::new`] is the one caller that supplies `WebOptions::default()`; a test passes
//!   `PrivateNetAllow::Hosts(["127.0.0.1"])` — a grant for one host, not `Any` — and everything
//!   else stays real. `host.rs`'s `the_default_egress_guards_the_private_network` still asserts the
//!   shipped default on the value, and
//!   [`the_default_egress_refuses_the_very_request_the_grant_admits`] asserts it a second and
//!   stronger way: the *same* projected operation, run under [`App::new`], is refused and the
//!   vendor records nothing. So the widening is proved to be the only reason the live test can
//!   send, which is what keeps it from quietly becoming the state a reader finds by accident.
//!
//! - **The origin is retargeted; nothing downstream of it is.** No shipped connector can be pointed
//!   at a loopback address, and that is deliberate rather than an oversight: nine carry a
//!   `{placeholder}` in their base URL, every one of them templates a *label* inside a fixed vendor
//!   suffix (`{subdomain}.zendesk.com`), and C-214's `Slot` guard exists specifically to stop a
//!   configuration value from moving a request to another host. Making a connector
//!   loopback-pointable through configuration would be re-opening that hole to test it.
//!
//!   So [`Retargeted`] rewrites **one string** — `https://api.openai.com` — in the `url` the pack
//!   hands the transport, and changes nothing else. The method, the path, the header the connector
//!   declares, the body's field set and its canonical JSON encoding, the credential's placement and
//!   its `Bearer ` prefix are all the shipped operation's, built by the shipped request path and
//!   dispatched through `Operation::execute`. The bound worth stating plainly: **this proves the
//!   pack's request survives the wire intact, not that `api.openai.com` answers it.** The live leg
//!   against a real vendor stays manual and stays recorded in `crates/connectors-api/README.md`.
//!
//! # The retarget moved from the artifact to the transport (C-538)
//!
//! It used to rewrite that string in the operation's **emitted Flux** and leak a doctored
//! `catalog::Operation`. `Operation::build_request` reads the canonical document since C-538, so a
//! doctored module changes nothing the pack builds — the request would have gone to the real
//! `api.openai.com` with a sentinel key, which is exactly what a green-looking test must never do.
//!
//! [`Egress`] is the seam the design already names for this: *"a dry-run that renders the request
//! instead of sending it, or a recorded fixture, without either forking the request path"*. So the
//! substitution happens there, one layer below the pack and one layer above the client, and the
//! operation under test is the **unmodified shipped entry**. What that costs is stated where it is
//! paid: [`the_vendor_receives_exactly_the_request_the_pack_built`] compares the received URL
//! against the built URL *with the same one substitution applied*, so the origin is the one field
//! this file does not prove survived the wire — and the path, the query, the method, the headers
//! and the body still are.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use std::future::Future;
use std::pin::Pin;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, Uri};
use catalog::OperationKey;
use connector_pack::{
    Configuration, CredentialRef, Egress, MemoryConfig, Operation, Secret, DEFAULT_SERVICE,
};
use connectors_api::App;
use flux_runtime::{Tool, ToolContext, ToolResult};
use flux_spec::ToolSpec;
use flux_system::net::PrivateNetAllow;
use flux_web::http::HttpRequestTool;
use flux_web::WebOptions;
use serde_json::{json, Value};

/// The connector this file drives. Chosen because it exercises all four fields at once — a `POST`,
/// a path, a header the module itself sets, and a JSON body — and because its base URL is a literal
/// rather than a template, so the retargeting is one substitution and no configuration is involved.
const OPERATION: &str = "openai-chat-completion";

/// The vendor origin that operation's own Flux names.
const VENDOR_ORIGIN: &str = "https://api.openai.com";

/// The connector's authority, which is the middle segment of its credential address.
const AUTHORITY: &str = "com.openai.api";

/// The tenant these tests store a credential for. Not a session tenant: nothing here goes through a
/// route, so nothing here needs a principal.
const TENANT: &str = "loopback-vendor-tenant";

/// An obviously-fake credential, long enough that flux's redactor will hold it — `add_secret` is a
/// documented no-op under six trimmed characters, and `connector-pack` refuses such a value rather
/// than sending it.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-openai-api-key";

/// Headers the HTTP client adds on the way out, which are the transport's and not the pack's.
///
/// Excluded from the equality below rather than asserted on: `host`, `content-length` and the
/// `accept*` pair are hyper's and reqwest's to decide, and pinning them here would turn a flux-web
/// or reqwest upgrade into a failure of this repository's test with nothing wrong in this
/// repository. Everything outside this list must be a header the pack authored, exactly.
///
/// **`user-agent` was on this list and came off it with C-223**, which is the entry worth explaining
/// because the list is otherwise about layers below this repository. It was here on a true premise —
/// no code in this repository set one — and the premise was the defect: neither `Client::builder()`
/// site in `codewandler-flux-web` 0.41.0 calls `ClientBuilder::user_agent` and reqwest sends no
/// default, so nothing was being excused and nothing arrived. Now `connector_pack` authors it during
/// request assembly, so it belongs in the equality like any other pack header — and the equality is
/// what proves the identity survives the wire byte-identically rather than merely being built.
const TRANSPORT_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "accept",
    "accept-encoding",
    "connection",
];

/// **One request, as the vendor received it.**
#[derive(Clone, Debug, PartialEq, Eq)]
struct Received {
    method: String,
    /// Reassembled from the `Host` header and the request target, because that is what the server
    /// actually sees: an origin-form HTTP/1.1 request carries the authority in a header and the
    /// path in the request line, and the absolute URL the pack built is the two put back together.
    url: String,
    headers: BTreeMap<String, String>,
    body: Option<String>,
}

/// A loopback HTTP server that records every request and answers each one the same way.
#[derive(Clone)]
struct Vendor {
    origin: String,
    received: Arc<Mutex<Vec<Received>>>,
}

impl Vendor {
    /// Start one on an ephemeral loopback port.
    async fn start() -> Self {
        let received: Arc<Mutex<Vec<Received>>> = Arc::new(Mutex::new(Vec::new()));
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("an ephemeral loopback port");
        let origin = format!("http://{}", listener.local_addr().expect("a bound address"));

        // A fallback rather than a route, so a request to the wrong path is recorded and asserted on
        // rather than answered with a 404 that says nothing about which path was wrong.
        let router = axum::Router::new()
            .fallback(record)
            .with_state(Arc::clone(&received));
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        Self { origin, received }
    }

    /// Everything this vendor has been sent, in order.
    fn received(&self) -> Vec<Received> {
        self.received.lock().expect("not poisoned").clone()
    }

    /// The one request this vendor was sent — and a failure if it was sent any other number.
    fn exactly_one(&self) -> Received {
        let received = self.received();
        assert_eq!(
            received.len(),
            1,
            "expected exactly one request to reach the vendor, got {}: {received:#?}",
            received.len()
        );
        received.into_iter().next().expect("one request")
    }
}

/// The vendor's whole surface: record what arrived, answer with a plausible completion.
async fn record(
    State(received): State<Arc<Mutex<Vec<Received>>>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> axum::Json<Value> {
    let authority = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();

    received.lock().expect("not poisoned").push(Received {
        method: method.as_str().to_owned(),
        url: format!("http://{authority}{uri}"),
        headers: headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    value.to_str().unwrap_or("<binary>").to_owned(),
                )
            })
            .collect(),
        body: (!body.is_empty()).then(|| String::from_utf8_lossy(&body).into_owned()),
    });

    axum::Json(json!({
        "id": "chatcmpl-loopback",
        "object": "chat.completion",
        "choices": [],
    }))
}

/// **The host's own transport, with the vendor origin — and only the vendor origin — rewritten.**
///
/// See this module's documentation for why the rewrite lives here and what it is bounded to. It is
/// a `dyn Tool` wrapping a `dyn Tool`, which is exactly what [`Egress`]'s typing exists to allow:
/// the pack composes the request, hands it to whatever transport the host bound, and this one
/// forwards it — same `ctx`, same params, one string changed — to the client the app configured. So
/// the SSRF guard, the private-network policy and the audit sink under test are still the app's.
///
/// `Tool` is `#[async_trait]`, and `connectors-api` does not depend on `async-trait`, so `execute`
/// is spelled in the desugared form the macro would have written. It is three lines of ceremony to
/// avoid a dependency this crate does not otherwise need.
struct Retargeted {
    inner: Arc<dyn Tool>,
    from: String,
    to: String,
}

impl Retargeted {
    /// Wrap `egress`, rewriting [`VENDOR_ORIGIN`] to `origin`.
    fn wrapping(egress: Egress, origin: &str) -> Egress {
        Egress::new(Arc::new(Retargeted {
            inner: Arc::clone(egress.tool()),
            from: VENDOR_ORIGIN.to_owned(),
            to: origin.to_owned(),
        }))
    }
}

impl Tool for Retargeted {
    fn spec(&self) -> ToolSpec {
        self.inner.spec()
    }

    fn execute<'inner, 'ctx, 'call>(
        &'inner self,
        ctx: &'ctx ToolContext,
        mut params: Value,
    ) -> Pin<Box<dyn Future<Output = flux_core::Result<ToolResult>> + Send + 'call>>
    where
        'inner: 'call,
        'ctx: 'call,
        Self: 'call,
    {
        // **The refusal that keeps this honest.** If the connector ever stops naming the vendor
        // origin, this rewrites nothing and the request goes to the real `api.openai.com` carrying
        // a sentinel key — a live call out of a test that would otherwise look green. It is a panic
        // rather than a `None` because there is no safe way to continue.
        let url = params
            .get("url")
            .and_then(Value::as_str)
            .expect("`http.request` is always given a url");
        assert!(
            url.contains(&self.from),
            "the request the pack built does not name `{}`, so this test would have sent it to \
             the real vendor: {url}",
            self.from
        );
        params["url"] = Value::String(url.replace(&self.from, &self.to));

        Box::pin(async move { self.inner.execute(ctx, params).await })
    }
}

/// The URL the vendor under test should see, given the URL the pack built.
fn retargeted(url: &str, origin: &str) -> String {
    url.replace(VENDOR_ORIGIN, origin)
}

/// The credential port, the configuration port and the projection, for one tenant.
///
/// The credential is stored first so that `build_authenticated_request` resolves a value rather
/// than refusing by address — which is what `host.rs` already asserts, and is not what this file is
/// about.
async fn projected(app: &App, egress: Egress, entry: &'static catalog::Operation) -> Operation {
    let reference = CredentialRef::new(TENANT, AUTHORITY, DEFAULT_SERVICE, "api_key")
        .expect("a well-formed credential address");
    app.put_secret(&reference, Secret::new(SENTINEL.to_owned()))
        .await
        .expect("the store accepts a value");

    Operation::project(
        entry,
        egress,
        app.credentials(TENANT).expect("a usable tenant id"),
        // Empty, deliberately: this connector's base URL carries no `{placeholder}`, so an
        // operation that asked for one would be a refusal here rather than a silent default.
        Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a usable tenant id"),
    )
    .expect("the shipped operation projects onto the host's egress")
}

/// The call this file makes, as a model would.
fn params() -> Value {
    json!({
        "model": "gpt-4o-mini",
        "messages": [{ "role": "user", "content": "one loopback request" }],
        "max_completion_tokens": 16,
    })
}

/// `name: value` with every name lowercased.
///
/// HTTP header names are case-insensitive and hyper normalises them, so the pack's `Authorization`
/// arrives as `authorization`. The *names* are compared case-insensitively for that reason and the
/// **values verbatim**, which is where a credential mangled in transit would show.
fn lowercased(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
        .collect()
}

/// The received headers the pack is answerable for — everything the transport did not add.
fn pack_authored(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter(|(name, _)| !TRANSPORT_HEADERS.contains(&name.as_str()))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// **The vendor received exactly the request the pack built.**
///
/// The first byte this repository has ever sent under test, and the assertion is equality on all
/// four fields rather than "a request arrived": a transport that dropped the body, lowercased a
/// path, re-based the URL or lost a header would each answer `200` from a real vendor, and each is
/// visible only here.
#[tokio::test]
async fn the_vendor_receives_exactly_the_request_the_pack_built() {
    let vendor = Vendor::start().await;

    // The one deliberate widening, and it is a grant for a single host rather than
    // `PrivateNetAllow::Any`. Everything else is `WebOptions::default()`.
    let app = App::with_web_options(
        env!("CARGO_MANIFEST_DIR"),
        WebOptions {
            private_net: PrivateNetAllow::Hosts(vec!["127.0.0.1".to_owned()]),
            ..WebOptions::default()
        },
    )
    .expect("the crate root exists");

    let entry = catalog::operation(OperationKey::id(OPERATION))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{OPERATION}`"));
    let operation = projected(
        &app,
        Retargeted::wrapping(app.egress(), &vendor.origin),
        entry,
    )
    .await;
    let ctx = app.context();
    let params = params();

    // What the pack says it will send. Building does not send — the two calls below produce one
    // request on the wire, which `Vendor::exactly_one` is what proves.
    let built = operation
        .build_authenticated_request(&ctx, &params)
        .await
        .expect("the pack builds and authenticates the request");

    let result = operation
        .execute(&ctx, params.clone())
        .await
        .expect("the request reaches the loopback vendor");

    let received = vendor.exactly_one();

    assert_eq!(
        received.method, built.method,
        "the method changed in flight"
    );
    // The origin is the one field this file rewrites; everything after it — the path, and the
    // query if the operation had one — is compared verbatim. See this module's documentation.
    assert_eq!(
        received.url,
        retargeted(&built.url, &vendor.origin),
        "the URL changed in flight beyond the origin this test retargets"
    );
    assert_eq!(received.body, built.body, "the body changed in flight");
    assert_eq!(
        pack_authored(&received.headers),
        lowercased(&built.headers),
        "the headers that arrived are not the headers the pack built"
    );

    // The four fields above are the acceptance; these two are the sanity that the exchange
    // completed rather than the server having recorded a request it then failed to answer.
    //
    // **Read off `view()`, not `content`** (C-403). Since flux-web 0.43 the canonical `content` is
    // the `{status, headers, body}` record and the flat block is the model-facing view; these two
    // want the block, and what the record carries is
    // [`the_response_comes_back_as_a_record_not_a_flat_string`]'s subject rather than this test's.
    assert!(
        result.view().starts_with("HTTP 200 OK"),
        "the vendor's response did not come back: {}",
        result.view()
    );
    assert!(
        result.view().contains("chatcmpl-loopback"),
        "the vendor's body did not come back: {}",
        result.view()
    );
    assert!(!result.is_error, "a 200 is not a tool error");
}

/// **What a caller gets back, pinned** (C-403).
///
/// The one assertion in this repository that a *consumer* of `connector-pack` can be broken by
/// without a compile error. `Operation::execute` returns the transport's [`ToolResult`] unchanged,
/// so the shape of that result is flux-web's to decide and this repository's to state — and a host
/// parsing the old flat block against the new record gets no type error, only a silent behaviour
/// change. So it is pinned here, where a real `HttpRequestTool` answers a real response.
///
/// **Since flux-web 0.43 the canonical `content` is the record `{status, headers, body}`**, JSON
/// encoded, with `body` *parsed* when the response is a JSON object or array. The flat
/// `HTTP {status}\n{headers}\n{body}` block survives as the model-facing `view`, which is why both
/// halves are asserted: a bump that shaped one and not the other would be a caller reading the wrong
/// one of two plausible strings.
///
/// The `content != view` assertion is the tripwire for the version this file used to run against.
/// `ToolResult::view()` falls back to `content` when no view is set, so a transport that shaped
/// nothing at all would satisfy every "the block is still there" assertion above it.
#[tokio::test]
async fn the_response_comes_back_as_a_record_not_a_flat_string() {
    let vendor = Vendor::start().await;
    let app = App::with_web_options(
        env!("CARGO_MANIFEST_DIR"),
        WebOptions {
            private_net: PrivateNetAllow::Hosts(vec!["127.0.0.1".to_owned()]),
            ..WebOptions::default()
        },
    )
    .expect("the crate root exists");

    let entry = catalog::operation(OperationKey::id(OPERATION))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{OPERATION}`"));
    let operation = projected(
        &app,
        Retargeted::wrapping(app.egress(), &vendor.origin),
        entry,
    )
    .await;
    let result = operation
        .execute(&app.context(), params())
        .await
        .expect("the request reaches the loopback vendor");

    let record: Value = serde_json::from_str(&result.content).unwrap_or_else(|error| {
        panic!(
            "the canonical `content` is not the `{{status, headers, body}}` record a caller \
             field-selects from ({error}); it is: {}",
            result.content
        )
    });

    assert_eq!(
        record.get("status"),
        Some(&json!(200)),
        "`status` must be the number a caller compares, not text: {record}"
    );
    assert!(
        record
            .get("headers")
            .is_some_and(|headers| headers.is_object()),
        "`headers` must be a map a caller can read a name out of: {record}"
    );
    // The whole point of the record: `$resp.body.id` rather than a substring search over a block.
    assert_eq!(
        record.pointer("/body/id"),
        Some(&json!("chatcmpl-loopback")),
        "the vendor's JSON body must arrive parsed under `body`: {record}"
    );
    assert_eq!(
        record.pointer("/body/object"),
        Some(&json!("chat.completion")),
        "the vendor's JSON body must arrive parsed under `body`: {record}"
    );

    // And the flat block is still reachable — as the model-facing view, and only there.
    assert!(
        result.view().starts_with("HTTP 200 OK"),
        "the flat block no longer survives as the model-facing view: {}",
        result.view()
    );
    assert_ne!(
        result.view(),
        result.content,
        "`view()` fell back to `content`, so the transport shaped nothing"
    );
    assert!(!result.is_error, "a 200 is not a tool error");
}

/// **The host identifies itself on the wire** (C-223).
///
/// Every other assertion in this file compares what arrived against what the pack built, which is
/// silent about a header *neither* of them carries. This one asserts on the wire directly: the
/// `User-Agent` the vendor received, read off the recorded request. Resend answers a request without
/// one with a `403` carrying a valid key, so the absence is a live failure that names the wrong
/// cause — an authorization status for a missing header.
///
/// Three claims, and they are separable on purpose:
///
/// 1. **A `User-Agent` arrived at all.** This is the one that was false before C-223: neither
///    `Client::builder()` site in `codewandler-flux-web` 0.41.0 calls `ClientBuilder::user_agent`,
///    `WebOptions` carries no field for one, and reqwest sends no default.
/// 2. **It names this software and its version**, rather than a browser or a bare product word. A
///    `User-Agent` that lies is worse than one that is absent, so the value is asserted against
///    `CARGO_PKG_VERSION` — which this crate and `connector-pack` both inherit from the workspace —
///    rather than against a literal that would drift at the next release.
/// 3. **The rehearsal agrees with the wire.** The same operation's [`Operation::dry_run`] is asked
///    for the same header and must report the byte-identical value. That is C-145's whole purpose,
///    and it is the property that decided *where* the identity lives: a header set by the transport
///    would be invisible here, because `DryRunTransport` holds no client at all.
#[tokio::test]
async fn the_vendor_receives_a_user_agent_that_names_this_software() {
    let vendor = Vendor::start().await;
    let app = App::with_web_options(
        env!("CARGO_MANIFEST_DIR"),
        WebOptions {
            private_net: PrivateNetAllow::Hosts(vec!["127.0.0.1".to_owned()]),
            ..WebOptions::default()
        },
    )
    .expect("the crate root exists");

    let entry = catalog::operation(OperationKey::id(OPERATION))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{OPERATION}`"));
    let operation = projected(
        &app,
        Retargeted::wrapping(app.egress(), &vendor.origin),
        entry,
    )
    .await;
    let params = params();

    operation
        .execute(&app.context(), params.clone())
        .await
        .expect("the request reaches the loopback vendor");

    let received = vendor.exactly_one();
    let user_agent = received.headers.get("user-agent").unwrap_or_else(|| {
        panic!(
            "the request left the host with no `User-Agent`, which Resend answers with a 403 \
             carrying a valid key; the vendor received {:?}",
            received.headers.keys().collect::<Vec<_>>()
        )
    });

    // **The product token, not the whole value.** An earlier revision of this asserted that the
    // value *contained* `flux-connectors`, and a mutation proved that worthless: the repository URL
    // in the trailing comment satisfies it, so `Mozilla/5.0 0.7.0 (+…/flux-connectors)` passed a
    // test whose entire purpose is refusing a `User-Agent` that lies. RFC 9110 §10.1.5 puts the
    // identity in the **first** product token and everything after it is commentary, so that is what
    // is asserted, whole and equal.
    let product = user_agent.split_whitespace().next().unwrap_or_default();
    assert_eq!(
        product,
        format!("flux-connectors/{}", env!("CARGO_PKG_VERSION")),
        "the `User-Agent`'s product token does not name this software and its version: \
         {user_agent:?}"
    );

    // The rehearsal is the wire, or C-145's transport is describing a call the host does not make.
    let rehearsed = operation
        .dry_run(&params)
        .expect("the shipped operation rehearses");
    assert_eq!(
        rehearsed.request().headers.get("User-Agent"),
        Some(user_agent),
        "the dry run reports a different `User-Agent` than the one the vendor received"
    );
}

/// **The shipped default refuses the very request the grant admits.**
///
/// `host.rs` asserts `WebOptions::default().private_net` is `PrivateNetAllow::None` on the value.
/// This asserts the consequence, on the same operation, the same ports and the same params as the
/// test above — so "the production guard is unchanged" is proved by observing it refuse rather than
/// by reading a field, and the widening above is proved to be the only reason that test can send.
///
/// The second assertion is the one that would catch the bad version of this: a guard that refused
/// *after* the request left would still return an error.
#[tokio::test]
async fn the_default_egress_refuses_the_very_request_the_grant_admits() {
    let vendor = Vendor::start().await;
    let app = App::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");

    let entry = catalog::operation(OperationKey::id(OPERATION))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{OPERATION}`"));
    let operation = projected(
        &app,
        Retargeted::wrapping(app.egress(), &vendor.origin),
        entry,
    )
    .await;
    let ctx = app.context();

    let error = operation
        .execute(&ctx, params())
        .await
        .expect_err("the shipped default must refuse a loopback address");

    assert!(
        error.to_string().contains("refusing to fetch private"),
        "the refusal is not the SSRF guard's: {error}"
    );
    assert!(
        vendor.received().is_empty(),
        "the guard refused after the bytes had already left: {:?}",
        vendor.received()
    );
}

/// **The seam accepts the concrete `HttpRequestTool`, not merely `dyn Tool`.**
///
/// `Egress` is typed as `Arc<dyn Tool>` so that `connector-pack` never links an HTTP client, and
/// the cost of that typing is that nothing in the pack's own tests can show the concrete type fits.
/// This constructs it directly — not through `App`, which would prove only that `App` compiles —
/// wraps it, and projects an **unmodified** shipped operation onto it.
///
/// No socket is opened: `build_request` is the request before it is sent.
#[tokio::test]
async fn a_shipped_operation_projects_onto_a_real_http_request_tool() {
    let egress = Egress::new(Arc::new(HttpRequestTool::new(&WebOptions::default())));
    assert_eq!(
        egress.tool().spec().name,
        "http.request",
        "the concrete transport is not flux's http.request"
    );

    let app = App::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
    let entry = catalog::operation(OperationKey::id(OPERATION))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{OPERATION}`"));

    let operation = Operation::project(
        entry,
        egress,
        app.credentials(TENANT).expect("a usable tenant id"),
        Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a usable tenant id"),
    )
    .expect("a shipped operation projects onto a real HttpRequestTool");

    let request = operation
        .build_request(&params())
        .expect("the shipped operation builds its request");

    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://api.openai.com/v1/chat/completions");
    assert_eq!(
        operation.egress().tool().spec().name,
        "http.request",
        "the projection did not keep the transport it was handed"
    );
}
