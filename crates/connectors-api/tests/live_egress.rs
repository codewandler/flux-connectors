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
//!   suffix (`{subdomain}.zendesk.com`), and C-214's `request::Slot` guard exists specifically to
//!   stop a configuration value from moving a request to another host. Making a connector
//!   loopback-pointable through configuration would be re-opening that hole to test it.
//!
//!   So [`retargeted_at`] rewrites **one string literal** — `https://api.openai.com` — in the
//!   operation's own emitted Flux, and changes nothing else. The method, the path, the header the
//!   module sets, the body's field set and its canonical JSON encoding, the credential's placement
//!   and its `Bearer ` prefix are all the shipped operation's, evaluated by the shipped request
//!   path. The bound worth stating plainly: **this proves the pack's request survives the wire
//!   intact, not that `api.openai.com` answers it.** The live leg against a real vendor stays
//!   manual and stays recorded in `crates/connectors-api/README.md`.
//!
//! The doctored-entry technique is not new here — `connector-pack`'s
//! `an_operation_with_no_declared_host_is_refused` leaks a modified copy of a shipped entry for the
//! same reason: `Operation::project` takes a `&'static catalog::Operation`, and a corrupt- or
//! variant-catalogue case is otherwise unreachable from a test.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, Uri};
use catalog::OperationKey;
use connector_pack::{
    Configuration, CredentialRef, Egress, MemoryConfig, Operation, Secret, DEFAULT_SERVICE,
};
use connectors_api::App;
use flux_runtime::Tool;
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
const TRANSPORT_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "accept",
    "accept-encoding",
    "user-agent",
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

/// The shipped operation, with its **origin and only its origin** pointed at `origin`.
///
/// See this module's documentation for why the rewrite exists and what it is bounded to. The
/// assertion before the substitution is the part that keeps it honest: if the emitter ever stops
/// spelling the vendor origin as one literal, this rewrites something else, and that must be a
/// failure rather than a request quietly going to the real vendor.
fn retargeted_at(id: &str, origin: &str) -> &'static catalog::Operation {
    let entry = catalog::operation(OperationKey::id(id))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
    assert_eq!(
        entry.flux.matches(VENDOR_ORIGIN).count(),
        1,
        "`{id}` no longer names `{VENDOR_ORIGIN}` exactly once in its own Flux, so this \
         retargeting is rewriting something other than the origin"
    );

    let flux: &'static str = Box::leak(entry.flux.replace(VENDOR_ORIGIN, origin).into_boxed_str());
    // The declared host follows the request, or `permission_subjects`' fallback would name the
    // vendor for a call that cannot reach it.
    let authority: &'static str = Box::leak(
        origin
            .trim_start_matches("http://")
            .to_owned()
            .into_boxed_str(),
    );
    let hosts: &'static [&'static str] = Box::leak(Box::new([authority]));

    let mut doctored = *entry;
    doctored.flux = flux;
    doctored.hosts = hosts;
    // `project` takes a `&'static` entry, which a doctored copy is not. Leaking one is what
    // `connector-pack`'s own catalogue-variant test does, for the same reason.
    Box::leak(Box::new(doctored))
}

/// The credential port, the configuration port and the projection, for one tenant.
///
/// The credential is stored first so that `build_authenticated_request` resolves a value rather
/// than refusing by address — which is what `host.rs` already asserts, and is not what this file is
/// about.
async fn projected(app: &App, entry: &'static catalog::Operation) -> Operation {
    let reference = CredentialRef::new(TENANT, AUTHORITY, DEFAULT_SERVICE, "api_key")
        .expect("a well-formed credential address");
    app.put_secret(&reference, Secret::new(SENTINEL.to_owned()))
        .await
        .expect("the store accepts a value");

    Operation::project(
        entry,
        app.egress(),
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

    let operation = projected(&app, retargeted_at(OPERATION, &vendor.origin)).await;
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
    assert_eq!(received.url, built.url, "the URL changed in flight");
    assert_eq!(received.body, built.body, "the body changed in flight");
    assert_eq!(
        pack_authored(&received.headers),
        lowercased(&built.headers),
        "the headers that arrived are not the headers the pack built"
    );

    // The four fields above are the acceptance; these two are the sanity that the exchange
    // completed rather than the server having recorded a request it then failed to answer.
    assert!(
        result.content.starts_with("HTTP 200 OK"),
        "the vendor's response did not come back: {}",
        result.content
    );
    assert!(
        result.content.contains("chatcmpl-loopback"),
        "the vendor's body did not come back: {}",
        result.content
    );
    assert!(!result.is_error, "a 200 is not a tool error");
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

    let operation = projected(&app, retargeted_at(OPERATION, &vendor.origin)).await;
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
