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

/// **A second resolve of the same credential registers nothing** (C-159, finding 3).
///
/// `Redactor::add_secret` pushes onto a `Vec` and dedupes nothing, and `redact` is linear in that
/// set — so a long-lived process making tens of thousands of credentialed calls grew the set by an
/// entry per call and paid for it on every scrub of every tool result. The pack now asks the
/// redactor whether it already holds the value and tells it only what it does not.
///
/// # The probe, and why it is honest
///
/// `flux_secret::Redactor` exposes no count: `values` is private and there is no `len`. What is
/// observable is that `redact` replaces **each** registered copy in turn and that its replacement
/// text `[redacted]` *contains* the word `redacted` — so a duplicate in the set nests the marker,
/// and `[[redacted]]` is a set of two while `[redacted]` is a set of one.
///
/// That makes the probe dependent on flux's replacement text, so the expectation is **measured**
/// from a control redactor rather than written out, and the third assertion checks that the probe
/// can see a duplicate at all. If a future flux makes duplicates invisible, this test says so
/// instead of quietly asserting nothing.
#[tokio::test]
async fn a_repeated_resolve_does_not_grow_the_registered_set() {
    /// A value the redaction marker contains. Not a token shape, and not a plausible credential —
    /// it is a probe, and the doc comment above is why it has to be this word.
    const PROBE: &str = "redacted";

    // The control: a redactor told the value exactly once, and then a second time.
    let control = context(Arc::new(Progress::default()));
    control.redactor.add_secret(PROBE.to_owned());
    let once = control.redactor.redact(PROBE);
    control.redactor.add_secret(PROBE.to_owned());
    assert_ne!(
        control.redactor.redact(PROBE),
        once,
        "the probe cannot distinguish one registration from two, so the assertion below would hold \
         against any implementation"
    );

    let credentials = Credentials::new(store_holding(PROBE).await, TENANT).expect("a tenant");
    let ctx = context(Arc::new(Progress::default()));
    let tool = projected("slack-chat-post-message", reflecting_egress(), credentials);

    for _ in 0..2 {
        tool.build_authenticated_request(&ctx, &post_message_params())
            .await
            .expect("the store holds the value");
    }

    assert_eq!(
        ctx.redactor.redact(PROBE),
        once,
        "a repeated resolve of one credential grew the redactor's registered set"
    );
}

// ---------------------------------------------------------------------------------------------
// The Basic user half, driven through the public entry point (C-198, unblocked by C-92)
//
// **These two tests moved here from `src/credentials.rs`, and the move is the point.** C-198 wrote
// them inside the crate over a `Box::leak`ed zendesk doctored with an authority, because at that
// time zendesk, jira and twilio — the only three connectors declaring a `BasicJoin` credential —
// all declared `authority: None`. `Credentials::reference` therefore refused with
// `NoCredentialAddress` before the configuration port was ever consulted, so the entire Basic
// branch of `auth::acquire` had no shipped consumer and could only be reached by faking one.
//
// C-92 gave zendesk `com.zendesk.api`. Nothing is doctored now: the provider, the operation, the
// credential, its `user_suffix: "/token"` and its `Placement::Header { Authorization, "Basic " }`
// are all read from the shipped catalogue, and the request is built through the public
// `Operation::build_authenticated_request` rather than by re-assembling its body in a test. That is
// what makes this an assertion about a connector a host could actually install.
//
// **What did not go away is the refusal**, and C-92's first pass deleted the only test of it on the
// grounds that "a wall that no longer exists is not behaviour to pin". The wall does still exist:
// `Provider::authority` is `Option<&'static str>` (`crates/catalog/src/lib.rs`), so a provider
// without one remains constructible and `Credentials::reference` still has to refuse it. What
// changed is only that no *shipped* provider reaches it — which is precisely the condition under
// which a fail-closed path stops being exercised by accident and starts needing its own test.
// [`a_provider_without_an_authority_is_refused_rather_than_addressed`] is that test.
// ---------------------------------------------------------------------------------------------

/// The tenant both ports below answer for.
const BASIC_TENANT: &str = "t-c198";

/// An obvious non-credential, long enough that the host's redactor actually holds it.
const BASIC_SECRET: &str = "SENTINEL-NOT-A-REAL-SECRET-C198";

/// The account identifier a tenant binds — the non-secret half, without zendesk's `/token`.
const BASIC_USER: &str = "ops@acme.test";

/// `base64("ops@acme.test/token:SENTINEL-NOT-A-REAL-SECRET-C198")`, computed independently of the
/// crate under test:
///
/// ```text
/// printf 'ops@acme.test/token:SENTINEL-NOT-A-REAL-SECRET-C198' | base64 -w0
/// ```
///
/// A literal rather than a call to the pack's own encoder, because an assertion computed the same
/// way as the code it checks would pass on any encoder, correct or not — and the `/token` suffix is
/// visible in the plaintext above, which is the part that has no other test.
const BASIC_EXPECTED: &str = "b3BzQGFjbWUudGVzdC90b2tlbjpTRU5USU5FTC1OT1QtQS1SRUFMLVNFQ1JFVC1DMTk4";

/// A configuration port holding zendesk's subdomain, and its user half only when `user` is set.
fn basic_configuration(user: Option<&str>) -> Configuration {
    // Keyed by service as well as by connector (C-197). zendesk declares one API surface, so both
    // bind under `DEFAULT_SERVICE` — but the service is named rather than elided, because the port
    // takes it for every field and a two-service connector would need one binding per service.
    let mut values = MemoryConfig::new().with_endpoint(
        BASIC_TENANT,
        "zendesk",
        DEFAULT_SERVICE,
        "subdomain",
        "acme",
    );
    if let Some(user) = user {
        values = values.with_username(
            BASIC_TENANT,
            "zendesk",
            DEFAULT_SERVICE,
            "zendesk.api_token",
            user,
        );
    }
    Configuration::new(Arc::new(values), BASIC_TENANT).expect("a valid tenant id")
}

/// A credential port holding [`BASIC_SECRET`] at zendesk's api-token address.
///
/// The address is spelled out rather than derived, and it is the authority C-92 declared:
/// `tenants/t-c198/com.zendesk.api/api_token`. If `providers/zendesk.toml` were ever repointed the
/// store would answer nothing here and both tests would fail — which is the intended coupling, since
/// repointing a published authority is exactly the change `AGENTS.md` forbids.
async fn basic_credentials() -> Credentials {
    let store = MemoryStore::new();
    store
        .put(
            &CredentialRef::new(
                BASIC_TENANT,
                "com.zendesk.api",
                DEFAULT_SERVICE,
                "api_token",
            )
            .expect("a valid address"),
            &Secret::new(BASIC_SECRET),
        )
        .await
        .expect("an in-memory put cannot fail");
    Credentials::new(Arc::new(store), BASIC_TENANT).expect("a valid tenant id")
}

/// zendesk's ticket read, projected over a reflecting egress and the two ports above.
async fn basic_tool(user: Option<&str>) -> Operation {
    let entry = catalog::operation(catalog::OperationKey::id("zendesk-ticket-show"))
        .expect("the shipped catalogue carries zendesk-ticket-show");
    Operation::project(
        entry,
        reflecting_egress(),
        basic_credentials().await,
        basic_configuration(user),
    )
    .expect("zendesk-ticket-show projects")
}

/// **The Basic user half reaches the header, and it comes from the configuration port.**
///
/// Driven through `build_authenticated_request`, so what is asserted is the composed request as it
/// would go out: the tenant's subdomain substituted into the URL, and
/// `Authorization: Basic base64("<user>/token:<secret>")` with the suffix the connector declares
/// rather than one a host was asked to know.
#[tokio::test]
async fn a_basic_user_half_reaches_the_header_from_the_configuration_port() {
    let ctx = context(Arc::new(Progress::default()));
    let tool = basic_tool(Some(BASIC_USER)).await;

    let request = tool
        .build_authenticated_request(&ctx, &json!({ "ticket_id": 1 }))
        .await
        .expect("the store holds the token and the port holds the user half");

    assert_eq!(
        request.url,
        "https://acme.zendesk.com/api/v2/tickets/1.json"
    );
    assert_eq!(
        request.headers.get("Authorization").map(String::as_str),
        Some(format!("Basic {BASIC_EXPECTED}").as_str()),
        "the Basic pair must join the configured user, the declared suffix and the stored secret"
    );
    // The assembled pair is what actually travels, and it contains neither the secret nor the user
    // half in the clear — so scrubbing the header for `BASIC_SECRET` would assert nothing. What has
    // to hold is that the redactor knows the **base64**, which is as good as the secret to anyone
    // holding it.
    assert_ne!(
        ctx.redactor.redact(BASIC_EXPECTED),
        BASIC_EXPECTED,
        "the base64 pair travels, so the redactor must already hold it"
    );
}

/// **And it refuses by name when the port has nothing bound.** Composing `base64("/token:…")`
/// instead would produce a header the vendor answers with a 401 that says nothing about what is
/// missing — and the refusal quotes `ZENDESK_USER`, which is what a zendesk operator has actually
/// seen this value called.
#[tokio::test]
async fn a_basic_credential_with_no_configured_user_is_refused_by_name() {
    let ctx = context(Arc::new(Progress::default()));
    let tool = basic_tool(None).await;

    let error = tool
        .build_authenticated_request(&ctx, &json!({ "ticket_id": 1 }))
        .await
        .expect_err("a Basic credential with no user half is not a credential");

    assert!(
        matches!(&error, Error::MissingCredentialConfig { credential, .. }
            if credential == "zendesk.api_token"),
        "{error}"
    );
    let rendered = error.to_string();
    assert!(rendered.contains("ZENDESK_USER"), "{rendered}");
    assert!(rendered.contains(BASIC_TENANT), "{rendered}");
    assert!(!rendered.contains(BASIC_SECRET), "{rendered}");
}

/// **A provider with no `authority` has no credential address, and the port refuses instead of
/// guessing one.**
///
/// This is the crate's fail-closed assertion on that path, and it is deliberately built rather than
/// taken from `providers/`: since C-92 every shipped provider declares an authority, so nothing in
/// the catalogue reaches this branch any more. That makes the test *more* necessary, not less —
/// `Provider::authority` is still `Option`, the loader still accepts a provider TOML that omits it,
/// and the failure this guards against is the one that looks like success: composing some fallback
/// address and sending a request under it.
///
/// `crates/connector-spec/tests/credential_paths.rs::the_three_outcomes_are_distinguishable` keeps
/// the same case alive one crate over, and for the same reason. Here the fixture is a `Copy` of the
/// shipped zendesk with the one field cleared — the same `Box::leak` technique `tool.rs` uses for a
/// host-less entry — so everything except the missing authority is real catalogue data, and the
/// refusal cannot be an artefact of an otherwise-synthetic provider.
#[tokio::test]
async fn a_provider_without_an_authority_is_refused_rather_than_addressed() {
    fn zendesk_without_an_authority() -> &'static catalog::Provider {
        let mut provider = *catalog::provider(catalog::ProviderKey::id("zendesk"))
            .expect("the shipped catalogue carries zendesk");
        assert!(
            provider.authority.is_some(),
            "zendesk is expected to ship an authority; clearing it below is what makes this a test"
        );
        provider.authority = None;
        Box::leak(Box::new(provider))
    }

    let provider = zendesk_without_an_authority();
    let credential = provider
        .credential("zendesk.api_token")
        .expect("zendesk declares its api token");
    let credentials = Credentials::new(store_with_the_sentinel().await, TENANT).expect("a tenant");

    let error = credentials
        .reference("zendesk-ticket-show", provider, credential)
        .expect_err("a connector with no authority has no credential address");

    assert!(
        matches!(&error, Error::NoCredentialAddress { credential, .. }
            if credential == "zendesk.api_token"),
        "{error}"
    );
    // The diagnostic has to name which fact is missing, or an operator cannot act on it — the
    // requirement `catalog::Provider::authority`'s own documentation states.
    let rendered = error.to_string();
    assert!(rendered.contains("zendesk"), "{rendered}");
    assert!(rendered.contains("zendesk-ticket-show"), "{rendered}");
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
