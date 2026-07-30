//! **The safety test of the credential port (C-116).**
//!
//! A credential is resolved in this crate, assembled in this crate, and placed on a request in this
//! crate. Every one of those steps is a place the value could reach a surface a human or a model
//! reads, and the defence is one call in one order: `ctx.redactor.add_secret(…)` **before** the
//! request is constructed, so a failure anywhere after that point cannot surface the value.
//!
//! # The four surfaces are flux's, not this repository's invention
//!
//! `Executor::dispatch` scrubs exactly four things through the context's own [`Redactor`], and this
//! file asserts against the same four (`codewandler-flux-runtime` 0.39.0):
//!
//! | surface | where flux scrubs it |
//! |---|---|
//! | `ToolResult::content` | `lib.rs:4006` |
//! | `ToolResult::view` | `lib.rs:4007` |
//! | the error a failed `execute` folds to | `lib.rs:4010` |
//! | a progress line | `ToolProgressReporter::report`, `lib.rs:260` |
//!
//! So the property under test is **not** "the pack scrubs its own output" — flux already does that,
//! and doing it twice would let the ordering rot unnoticed. It is that **the redactor already holds
//! the value at the moment any of the four is produced**. Against an implementation that builds the
//! `Authorization` header without registering the secret, every scrub below is the identity
//! function and the sentinel survives all four.
//!
//! # The transport is deliberately adversarial
//!
//! The stand-in egress **reflects the request it was handed** into its result. A real
//! `http.request` returns the *response*, so it would carry no request header at all and the
//! assertions would pass for the wrong reason. Reflecting it is the worst case a substitute
//! transport can be — a dry-run renderer, a recorded fixture, a vendor error quoting the request —
//! and it is the case the redactor exists for.

use std::sync::{Arc, Mutex};

use connector_pack::{Credentials, Egress, Error, Operation, DEFAULT_SERVICE};
use connector_secrets::{CredentialRef, MemoryStore, Secret, SecretStore};
use flux_runtime::{RuntimeTurnContext, Tool, ToolContext, ToolProgress, ToolProgressSink};
use flux_system::{System, Workspace};
use serde_json::{json, Value};

/// An obvious non-credential. Nothing here may commit a value shaped like a real token — and it
/// deliberately carries none of `flux_secret`'s known prefixes (`xoxb-`, `sk-`, `ghp_`, …), so a
/// pass cannot come from flux's prefix-shaped redaction instead of from registration.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-C116";

/// The tenant every reference below is addressed under.
const TENANT: &str = "t-c116";

/// A recording progress sink, so the fourth surface is asserted end to end rather than by analogy.
#[derive(Default)]
struct Progress(Mutex<Vec<String>>);

impl ToolProgressSink for Progress {
    fn emit(&self, progress: ToolProgress) {
        self.0.lock().unwrap().push(progress.line);
    }
}

/// A `ToolContext` with a progress sink installed.
///
/// The workspace root is this crate's own directory: `System` requires one that exists, and nothing
/// in this pack ever reaches the filesystem through it.
fn context(progress: Arc<Progress>) -> ToolContext {
    let workspace = Workspace::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
    let mut ctx = ToolContext::new(Arc::new(System::new(workspace)));
    ctx.set_runtime_turn_context(RuntimeTurnContext::new().with_tool_progress_sink(progress));
    ctx
}

/// A store holding the sentinel at Slack's bot-token address for [`TENANT`].
async fn store_with_the_sentinel() -> Arc<dyn SecretStore> {
    let store = MemoryStore::new();
    store
        .put(&slack_bot_token(), &Secret::new(SENTINEL))
        .await
        .expect("an in-memory put cannot fail");
    Arc::new(store)
}

/// `tenants/t-c116/com.slack.api/bot_token` — C-90's addressing, not a second scheme.
fn slack_bot_token() -> CredentialRef {
    CredentialRef::new(TENANT, "com.slack.api", DEFAULT_SERVICE, "bot_token")
        .expect("a valid address")
}

/// An egress that reflects the request it was handed. See this module's documentation.
fn reflecting_egress() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        stand_in_spec(),
        |params: Value| async move { Ok(json!({ "reflected": params })) },
    ))
}

/// An egress that fails, quoting the request — the failure path flux folds to `ToolResult::error`.
fn failing_egress() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        stand_in_spec(),
        |params: Value| async move { Err(format!("the vendor refused: {params}")) },
    ))
}

fn stand_in_spec() -> flux_spec::ToolSpec {
    flux_spec::ToolSpec {
        name: "http.request".into(),
        description: "a stand-in that reflects the request it was handed".into(),
        input_schema: json!({"type": "object"}),
        output_schema: None,
        effects: vec![flux_spec::Effect::Network],
        risk: flux_spec::Risk::Medium,
        idempotency: flux_spec::Idempotency::NonIdempotent,
        access: vec![flux_spec::AccessKind::Network],
        group: None,
    }
}

fn projected(id: &str, http: Egress, credentials: Credentials) -> Operation {
    let entry = catalog::operation(catalog::OperationKey::id(id))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
    Operation::project(entry, http, credentials).unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

fn post_message_params() -> Value {
    json!({ "channel": "C0FLUX", "text": "hello", "thread_ts": null })
}

/// **The story's named test.** A credential reaches the request and reaches nothing else.
#[tokio::test]
async fn a_credential_never_reaches_a_surface() {
    let credentials = Credentials::new(store_with_the_sentinel().await, TENANT).expect("a tenant");
    let progress = Arc::new(Progress::default());
    let ctx = context(progress.clone());

    let tool = projected("slack-chat-post-message", reflecting_egress(), credentials);
    let result = tool
        .execute(&ctx, post_message_params())
        .await
        .expect("the stand-in transport answers");

    // The control: the credential *did* reach the request. Without this the four assertions below
    // would pass against a pack that applies no credential at all, which is exactly the state this
    // story inherited from C-115.
    assert!(
        result.content.contains("Bearer") && result.content.contains(SENTINEL),
        "the reflected request carries no assembled credential, so the assertions below prove \
         nothing: {}",
        result.content
    );

    // Surface 1 and 2 — `ToolResult::content` and `::view`, scrubbed as `Executor::dispatch` does.
    assert!(
        !ctx.redactor.redact(&result.content).contains(SENTINEL),
        "the sentinel survived into the tool result"
    );
    assert!(
        !ctx.redactor
            .redact(result.view.as_deref().unwrap_or_default())
            .contains(SENTINEL),
        "the sentinel survived into the model-facing view"
    );

    // Surface 3 — the error a failed dispatch folds to.
    let failing = projected(
        "slack-chat-post-message",
        failing_egress(),
        Credentials::new(store_with_the_sentinel().await, TENANT).expect("a tenant"),
    );
    let failed = failing
        .execute(&ctx, post_message_params())
        .await
        .expect("a handler error folds to a soft result rather than an `Err`");
    assert!(
        !ctx.redactor.redact(&failed.content).contains(SENTINEL),
        "the sentinel survived into the failure path"
    );

    // Surface 4 — a progress line, through the only handle a tool can reach a sink by.
    ctx.progress_reporter("slack.chat.post.message")
        .expect("a sink is installed")
        .report(&format!("sending with {SENTINEL}"));
    let lines = progress.0.lock().unwrap().clone();
    assert!(!lines.is_empty(), "the sink recorded nothing");
    for line in lines {
        assert!(
            !line.contains(SENTINEL),
            "the sentinel survived into a progress line: {line}"
        );
    }
}

/// The ordering the acceptance names, asserted on its own: registration happens **before** the
/// request is constructed, so a failure *between* construction and dispatch cannot surface the
/// value.
///
/// The call below omits a declared parameter, so `build_request` refuses and no request is ever
/// built — and the redactor must already know the value at that point. An implementation that
/// registers after building fails here while passing every assertion above.
///
/// The refusal comes back as an `Err`, not a soft result: this pack's [`Error`] converts to
/// `flux_core::Error::Config`, and only a *handler's* failure inside `http.request` folds to
/// `ToolResult::error`. That distinction is exactly why the ordering matters — the window between
/// "credentials resolved" and "the request exists" is a window in which the only thing that can
/// happen is a failure.
#[tokio::test]
async fn the_redactor_knows_the_value_before_the_request_is_constructed() {
    let credentials = Credentials::new(store_with_the_sentinel().await, TENANT).expect("a tenant");
    let ctx = context(Arc::new(Progress::default()));

    let tool = projected("slack-chat-post-message", reflecting_egress(), credentials);
    let error = tool
        .execute(&ctx, json!({}))
        .await
        .expect_err("an omitted parameter is not a request");
    assert!(
        error.to_string().contains("channel"),
        "the refusal must name the parameter, not the credential: {error}"
    );

    assert!(
        !ctx.redactor.redact(SENTINEL).contains(SENTINEL),
        "the request failed to build and the redactor had never been told the value"
    );
    // And the refusal itself carries nothing. Belt and braces: the redactor is the guarantee, but an
    // error that quoted a resolved value would be a second surface nobody scrubs on this path.
    assert!(!error.to_string().contains(SENTINEL), "{error}");
}

/// A missing credential names the address that was not found, and **no request is sent**.
#[tokio::test]
async fn a_missing_credential_names_its_address_and_sends_nothing() {
    let credentials = Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a tenant");
    let ctx = context(Arc::new(Progress::default()));

    let tool = projected("slack-chat-post-message", reflecting_egress(), credentials);
    let error = tool
        .build_authenticated_request(&ctx, &post_message_params())
        .await
        .expect_err("an empty store cannot authenticate the call");

    let rendered = error.to_string();
    assert!(
        matches!(&error, Error::MissingCredential { .. }),
        "{rendered}"
    );
    assert!(
        rendered.contains("tenants/t-c116/com.slack.api/bot_token"),
        "the error must name the address that was not found: {rendered}"
    );
    assert!(
        rendered.contains("slack-chat-post-message"),
        "the error must name the operation: {rendered}"
    );
}
