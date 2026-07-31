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
//! Four is the complete set: `ToolResult` has exactly three fields, and the progress line is the only
//! other thing a tool can put in front of a reader. There is no fifth to add.
//!
//! # Two ways this file used to prove less than it claimed (C-152)
//!
//! **A `view` of `None` redacts to `""`.** `flux_runtime::tool_fn` builds every result with
//! `ToolResult::ok` (`fn_tool.rs:107`), so the second assertion was scrubbing an empty string. The
//! stand-in below is a `Tool` of its own that answers with `ToolResult::ok_view`, and the surface is
//! now asserted with a control the way the first one always was.
//!
//! **`add_secret` is a no-op under six trimmed characters** (`codewandler-flux-secret-1.0.1`,
//! `lib.rs:198`), so for a short credential registration succeeded and redaction never happened. The
//! pack now refuses such a credential rather than sending it, and
//! [`a_credential_too_short_to_redact_is_refused_rather_than_sent`] is that case.
//!
//! # The transport is deliberately adversarial
//!
//! The stand-in egress **reflects the request it was handed** into its result. A real
//! `http.request` returns the *response*, so it would carry no request header at all and the
//! assertions would pass for the wrong reason. Reflecting it is the worst case a substitute
//! transport can be — a dry-run renderer, a recorded fixture, a vendor error quoting the request —
//! and it is the case the redactor exists for.

use std::sync::{Arc, Mutex};

use connector_pack::{
    Configuration, Credentials, Egress, Error, MemoryConfig, Operation, DEFAULT_SERVICE,
};
use connector_secrets::{CredentialRef, MemoryStore, Secret, SecretStore};
use flux_runtime::{
    RuntimeTurnContext, Tool, ToolContext, ToolProgress, ToolProgressSink, ToolResult,
};
use flux_system::{System, Workspace};
use serde_json::{json, Value};

/// An obvious non-credential. Nothing here may commit a value shaped like a real token — and it
/// deliberately carries none of `flux_secret`'s known prefixes (`xoxb-`, `sk-`, `ghp_`, …), so a
/// pass cannot come from flux's prefix-shaped redaction instead of from registration.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-C116";

/// **Five characters, and that is the whole point.** `Redactor::add_secret` stores a value only when
/// it is at least six characters once trimmed (`codewandler-flux-secret-1.0.1/src/lib.rs:198`), so
/// registering this one *succeeds and redacts nothing* — the case C-152 found, where the code was
/// correct about what it did and the prose was wrong about what that meant.
///
/// A word rather than a token shape, for the same reason [`SENTINEL`] is: nothing here may commit a
/// value that reads as a credential, and at this length no `SENTINEL-NOT-A-REAL-…` spelling fits.
const SHORT_SENTINEL: &str = "SHORT";

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
    store_holding(SENTINEL).await
}

/// A store holding `value` at Slack's bot-token address for [`TENANT`].
async fn store_holding(value: &str) -> Arc<dyn SecretStore> {
    let store = MemoryStore::new();
    store
        .put(&slack_bot_token(), &Secret::new(value))
        .await
        .expect("an in-memory put cannot fail");
    Arc::new(store)
}

/// `tenants/t-c116/com.slack.api/bot_token` — C-90's addressing, not a second scheme.
fn slack_bot_token() -> CredentialRef {
    CredentialRef::new(TENANT, "com.slack.api", DEFAULT_SERVICE, "bot_token")
        .expect("a valid address")
}

/// An egress that reflects the request it was handed into **both** result surfaces.
///
/// `flux_runtime::tool_fn` cannot be used for this: it builds every result with `ToolResult::ok`,
/// which leaves `view: None` (`flux-runtime-0.39.0/src/fn_tool.rs:107`), so a `view` assertion
/// against it redacts an empty string and holds nothing (C-152, finding 3). `ToolResult::ok_view`
/// carries a real one, and a tool that reports its own view is not exotic — it is what any tool with
/// a model-facing rendering does.
struct Reflecting;

#[async_trait::async_trait]
impl Tool for Reflecting {
    fn spec(&self) -> flux_spec::ToolSpec {
        stand_in_spec()
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> flux_core::Result<ToolResult> {
        Ok(ToolResult::ok_view(
            json!({ "reflected": params }).to_string(),
            format!("the request as it went out: {params}"),
        ))
    }
}

/// An egress that reflects the request it was handed. See this module's documentation.
fn reflecting_egress() -> Egress {
    Egress::new(Arc::new(Reflecting))
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
    Operation::project(entry, http, credentials, configuration())
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

/// An **empty** configuration port, bound to the same tenant as the credential port (C-193).
///
/// Every operation this file drives is `slack-chat-post-message`, whose base URL is the literal
/// `https://slack.com` — it names no endpoint variable, so there is nothing to configure and an
/// empty port is the honest binding rather than a shortcut. What this file asserts is the
/// *credential* path, and a configuration value present here would only obscure which port a value
/// came from.
fn configuration() -> Configuration {
    Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant id")
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
    // The same control for the second surface, and it is the one C-152 found missing: a `view` that
    // is `None` redacts to `""`, which passes the assertion below while asserting nothing at all.
    let view = result
        .view
        .as_deref()
        .expect("the stand-in carries a view, or the assertion below is vacuous");
    assert!(
        view.contains("Bearer") && view.contains(SENTINEL),
        "the reflected view carries no assembled credential: {view}"
    );

    // Surface 1 and 2 — `ToolResult::content` and `::view`, scrubbed as `Executor::dispatch` does.
    assert!(
        !ctx.redactor.redact(&result.content).contains(SENTINEL),
        "the sentinel survived into the tool result"
    );
    assert!(
        !ctx.redactor.redact(view).contains(SENTINEL),
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

/// **A credential the host's redactor would silently decline to hold is refused, not sent.**
///
/// `Redactor::add_secret` is a no-op under six trimmed characters, so for a value this short every
/// one of the four scrubs above is the identity function and the guarantee the surrounding
/// documentation states does not hold. The decision C-152 records in
/// [the design](../../../docs/designs/connector-tool-pack.md) is to refuse at resolve time: a
/// credential the host cannot protect is one it should not send, and a five-character API token is a
/// misconfiguration long before it is a credential.
///
/// The `Ok` arm is the assertion that matters. Before the refusal existed this call *succeeded*, and
/// the value reached `ToolResult::content` through a redactor that had been told about it and had
/// dropped it on the floor — which is why the failure message quotes the **scrubbed** content: the
/// sentinel being visible there is the whole defect in one line.
#[tokio::test]
async fn a_credential_too_short_to_redact_is_refused_rather_than_sent() {
    let credentials =
        Credentials::new(store_holding(SHORT_SENTINEL).await, TENANT).expect("a tenant");
    let ctx = context(Arc::new(Progress::default()));

    let tool = projected("slack-chat-post-message", reflecting_egress(), credentials);
    let outcome = tool.execute(&ctx, post_message_params()).await;

    if let Ok(result) = &outcome {
        let scrubbed = ctx.redactor.redact(&result.content);
        assert!(
            !scrubbed.contains(SHORT_SENTINEL),
            "a credential too short for the redactor to hold was sent anyway, and survived every \
             scrub into the tool result: {scrubbed}"
        );
    }

    let error = outcome.expect_err("a credential the host cannot redact is not sent");
    let rendered = error.to_string();
    assert!(
        rendered.contains("slack.bot_token") && rendered.contains(TENANT),
        "the refusal must name the credential it could not protect: {rendered}"
    );
    assert!(
        !rendered.contains(SHORT_SENTINEL),
        "the refusal quoted the value it refused: {rendered}"
    );
}

/// **Why the Basic half is not driven end to end from this file, pinned as behaviour (C-198).**
///
/// `zendesk`, `jira` and `twilio` are the three connectors declaring a `BasicJoin` credential, and
/// all three declare `authority: None` — so `Credentials::reference` refuses before the
/// configuration port is ever consulted, and no shipped connector can reach the Basic assembly
/// through `Operation::build_authenticated_request`. That is C-92's gap, and `AGENTS.md` records the
/// refusal as the correct answer: a request sent without the credential it declares is the failure
/// worth preventing, and the diagnostic must name *which* fact is missing.
///
/// So this asserts the wall rather than pretending it is not there. Everything else is supplied —
/// the subdomain is configured and the store holds a token — so the only missing fact is the
/// authority, and the refusal has to say so. The full Basic assertion meanwhile lives in
/// `src/credentials.rs::a_basic_user_half_reaches_the_header_from_the_configuration_port`, which
/// doctors a provider to get past this point; the day C-92 gives zendesk an authority, that test
/// belongs here instead.
#[tokio::test]
async fn a_basic_connector_refuses_because_it_has_no_credential_address() {
    let configuration = Configuration::new(
        Arc::new(MemoryConfig::new().with_endpoint(TENANT, "zendesk", "subdomain", "acme")),
        TENANT,
    )
    .expect("a valid tenant id");
    let entry = catalog::operation(catalog::OperationKey::id("zendesk-ticket-show"))
        .expect("the shipped catalogue carries zendesk-ticket-show");
    let tool = Operation::project(
        entry,
        reflecting_egress(),
        Credentials::new(store_with_the_sentinel().await, TENANT).expect("a tenant"),
        configuration,
    )
    .expect("zendesk-ticket-show projects");

    let error = tool
        .build_authenticated_request(
            &context(Arc::new(Progress::default())),
            &json!({ "ticket_id": 1 }),
        )
        .await
        .expect_err("a connector with no authority has no credential address");

    assert!(
        matches!(&error, Error::NoCredentialAddress { credential, .. }
            if credential == "zendesk.api_token"),
        "{error}"
    );
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
