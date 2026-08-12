//! **The published plan seam** — the surface a consumer that owns its own `Tool` projection uses
//! (C-553).
//!
//! Exchange's X-156 read the 0.23.0 sources and found the seam missing: every input to
//! `connector_resolve::resolve` was produced only by `pub(crate)` paths here, dispatch was
//! `pub(crate)`, and the one public plan-deriving function returned `plan.request` and dropped
//! `permission_subjects` and `redactions`. This file is the consumer-shaped usage that closes it,
//! written the way an Exchange invoke path would write it.
//!
//! # What is asserted, and why each line is here rather than in a doc comment
//!
//! | | the claim |
//! |---|---|
//! | [`the_plan_seam_is_the_same_derivation_the_tool_path_dispatches`] | the plan is published, not re-derived: the wrapper's request is `plan.request`, byte for byte |
//! | [`a_consumer_dispatches_the_plan_without_unwrapping_the_transport`] | a bound `Egress` accepts a plan-derived request, so `Egress::tool()` stays refusable |
//! | [`the_redactor_already_holds_every_string_the_plan_names`] | the enforcement ordering survives the seam — registration happens before the plan exists, not as advice attached to it |
//! | [`nothing_the_plan_seam_publishes_prints_a_credential`] | `SensitiveText`/redacted-`Debug` discipline over every newly public item |
//!
//! **This file never names `.tool()`, and that is the point rather than an omission.** Exchange
//! refuses that spelling in every file with no exception list
//! (`no_second_request_path.rs::UNWRAPS_THE_TRANSPORT`), so a dispatch route that needed it would be
//! a route Exchange cannot take. Everything below reaches the transport through
//! [`Egress::send`](connector_pack::Egress::send) instead.

use std::sync::{Arc, Mutex};

use catalog::OperationKey;
use connector_pack::{
    Configuration, Credentials, Egress, MemoryConfig, MemoryStore, Operation, RequestPlan, Secret,
    SecretStore,
};
use flux_runtime::{Tool, ToolContext, ToolResult};
use flux_system::{System, Workspace};
use serde_json::{json, Value};

/// A value no vendor issued, long enough for flux's redactor to hold, and recognisable in a diff.
///
/// It deliberately carries none of `flux_secret`'s known prefixes (`xoxb-`, `sk-`, `ghp_`, …), so a
/// redaction assertion below cannot pass on flux's shape-matching pass instead of on the
/// registration this seam is supposed to have performed.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-CREDENTIAL-C553";

/// The tenant both ports answer for.
const TENANT: &str = "t-c553";

/// The one operation this file drives. A templated host, one path parameter, one bearer credential —
/// the smallest shape that exercises configuration substitution, parameter placement, credential
/// resolution and header placement at once.
const OPERATION: &str = "zendesk-ticket-show";

/// Zendesk's credential is a Basic join, so the tenant supplies the user half through the
/// configuration port (C-193) and the value that travels is `base64(user/token:secret)`.
const ZENDESK_USER: &str = "agent@example.test";

/// A `ToolContext` over this crate's own directory. `System` requires a workspace root that exists;
/// nothing on this path reaches the filesystem through it.
fn context() -> ToolContext {
    let workspace = Workspace::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
    ToolContext::new(Arc::new(System::new(workspace)))
}

/// A transport that records what it was handed and answers with a fixed result.
///
/// It reflects nothing, deliberately: this file asserts on the *plan*, and a transport that echoed
/// the request would let a redaction assertion pass on the echo rather than on the registration.
#[derive(Default)]
struct Recorder(Mutex<Vec<Value>>);

#[async_trait::async_trait]
impl Tool for Recorder {
    fn spec(&self) -> flux_spec::ToolSpec {
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a recording stand-in; this crate links no client".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        }
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> flux_core::Result<ToolResult> {
        self.0.lock().unwrap().push(params);
        Ok(ToolResult::ok("carried"))
    }
}

/// The ports a host binds, and the operation projected onto them.
///
/// This is the whole of the consumer's setup, and it is deliberately written out rather than hidden
/// behind a helper crate: everything it names — `Egress::new`, `Credentials::new`,
/// `Configuration::new`, `Operation::project` — was already public before C-553. What was missing
/// was the two lines each test below adds.
async fn projected(recorder: Arc<Recorder>) -> (Operation, Egress) {
    let entry = catalog::operation(OperationKey::id(OPERATION)).expect("a shipped operation");
    let provider = catalog::provider(catalog::ProviderKey::id(entry.provider)).expect("its vendor");

    let egress = Egress::new(recorder as Arc<dyn Tool>);

    let store = Arc::new(MemoryStore::new());
    let credentials =
        Credentials::new(store.clone() as Arc<dyn SecretStore>, TENANT).expect("a valid tenant id");
    for credential in provider.auth {
        let reference = credentials
            .reference(entry.id, provider, credential)
            .expect("the connector declares an authority");
        store
            .put(&reference, &Secret::new(SENTINEL))
            .await
            .expect("an in-memory put cannot fail");
    }

    let configuration = Configuration::new(
        Arc::new(
            MemoryConfig::new()
                .with_endpoint(TENANT, entry.provider, entry.service, "subdomain", "acme")
                .with_username(
                    TENANT,
                    entry.provider,
                    entry.service,
                    "zendesk.api_token",
                    ZENDESK_USER,
                ),
        ),
        TENANT,
    )
    .expect("a valid tenant id");

    let operation = Operation::project(entry, egress.clone(), credentials, configuration)
        .expect("the shipped operation projects");
    (operation, egress)
}

fn params() -> Value {
    json!({ "ticket_id": 35436 })
}

/// **Published, not re-derived.** The wrapper's request is the plan's request, byte for byte,
/// because it *is* the plan's request — [`Operation::build_authenticated_request`] returns
/// `plan.request` from the very call this seam publishes.
///
/// Asserted rather than assumed because the failure it guards against is silent: a second derivation
/// added beside this one would keep both signatures, keep both green in isolation, and put two
/// answers in front of one vendor. The catalogue-wide form of this claim is
/// `catalogue_differential.rs`; this is the unit-level statement of the same property, on the seam
/// itself.
#[tokio::test]
async fn the_plan_seam_is_the_same_derivation_the_tool_path_dispatches() {
    let (operation, _) = projected(Arc::new(Recorder::default())).await;
    let ctx = context();

    let plan: RequestPlan = operation
        .build_request_plan(&ctx, &params())
        .await
        .expect("the plan derives");
    let wrapper = operation
        .build_authenticated_request(&ctx, &params())
        .await
        .expect("the wrapper derives");

    assert_eq!(plan.request, wrapper);
    assert_eq!(plan.request.method, "GET");
    assert_eq!(
        plan.request.url,
        "https://acme.zendesk.com/api/v2/tickets/35436"
    );

    // The subject is the URL **before** any credential was placed — the value a host's network
    // policy, approval prompt and evidence record quote.
    assert_eq!(
        plan.permission_subjects,
        vec!["https://acme.zendesk.com/api/v2/tickets/35436".to_string()]
    );
    assert_eq!(
        plan.permission_subjects,
        operation.permission_subjects(&params()),
        "the plan's subjects and the projected tool's must be one answer, not two"
    );

    // And the redaction set names what actually travels: Basic joins the user half in, so the
    // string on the wire is the base64 rather than the stored value.
    let travelling: Vec<&str> = plan
        .redactions
        .iter()
        .map(connector_pack::SensitiveText::expose_secret)
        .collect();
    assert_eq!(travelling.len(), 1, "{travelling:?}");
    assert_ne!(travelling[0], SENTINEL, "a Basic join transforms the value");
    assert!(
        plan.request.headers["Authorization"].ends_with(travelling[0]),
        "the redaction set must name the string the header carries"
    );
}

/// **A consumer dispatches the plan it was handed, through the `Egress` it bound.**
///
/// This is the dispatch-seam decision executed: `connector-pack` publishes
/// [`Egress::send`](connector_pack::Egress::send) so a consumer that owns its own `Tool` projection
/// never has to reach the `Arc<dyn Tool>` inside the newtype. The alternative — leaving dispatch
/// behind the `Tool` projection — would have left `Egress::tool()` as the only public route to the
/// transport, and that spelling is refused in every Exchange file.
///
/// Note what is *not* asserted: that the consumer may edit the plan. It may not. It carries
/// `plan.request` across and dispatches it; a consumer that composed one has become the second
/// request path this family already rejected.
#[tokio::test]
async fn a_consumer_dispatches_the_plan_without_unwrapping_the_transport() {
    let recorder = Arc::new(Recorder::default());
    let (operation, egress) = projected(recorder.clone()).await;
    let ctx = context();

    let plan = operation
        .build_request_plan(&ctx, &params())
        .await
        .expect("the plan derives");
    let expected = plan.request.to_params();

    let result = egress
        .send(&ctx, plan.request)
        .await
        .expect("the bound transport carries it");

    assert_eq!(result.content, "carried");
    let carried = recorder.0.lock().unwrap().clone();
    assert_eq!(
        carried,
        vec![expected],
        "the transport must receive exactly the params the plan's request renders"
    );
}

/// **The enforcement ordering survives the seam.**
///
/// `RequestPlan::redactions` documents itself as *a requirement, not a record* — a consumer must
/// register the strings before it formats, logs or dispatches the plan. That is the contract for a
/// consumer of `connector-resolve` alone. Reaching the plan through **this** crate is stronger, and
/// the difference is what the Acceptance means by "the same enforcement topology": every value was
/// resolved and registered with `ctx.redactor` before a request existed at all, so the plan arrives
/// with the guarantee already discharged.
///
/// Asserted through the redactor rather than by reading the code, and asserted with a control: an
/// unregistered string of the same shape must survive the same scrub, or the assertion would pass
/// against a redactor that scrubs everything.
#[tokio::test]
async fn the_redactor_already_holds_every_string_the_plan_names() {
    let (operation, _) = projected(Arc::new(Recorder::default())).await;
    let ctx = context();

    let plan = operation
        .build_request_plan(&ctx, &params())
        .await
        .expect("the plan derives");

    for text in &plan.redactions {
        let value = text.expose_secret();
        assert_eq!(
            ctx.redactor.redact(value),
            ctx.redactor.redact(&format!("x{value}"))[1..],
            "`{value}` scrubs the same whether or not it is preceded by other text"
        );
        assert!(
            !ctx.redactor.redact(value).contains(value),
            "the plan names a string the redactor was never told about"
        );
    }

    // The whole authenticated header, as it goes out, is covered too — the prefix surrounds the
    // value rather than transforming it, so a redactor holding the bare form scrubs the header.
    let header = &plan.request.headers["Authorization"];
    assert!(
        !ctx.redactor.redact(header).contains(header.as_str()),
        "the Authorization header survives the host's own scrub"
    );

    // **The control.** A string the seam never resolved must pass through untouched, or the four
    // assertions above would hold against a redactor that scrubs indiscriminately.
    let unregistered = "UNREGISTERED-NOT-A-REAL-CREDENTIAL-C553";
    assert_eq!(ctx.redactor.redact(unregistered), unregistered);
}

/// **No secret-bearing value gains a printable path.**
///
/// Every item C-553 makes public is checked here rather than trusted to the type it borrowed the
/// property from: `connector-pack` is published, so each of these is API forever and a later
/// refactor that swapped one for a `#[derive(Debug)]` struct would be a permanent leak.
///
/// | newly public | posture |
/// |---|---|
/// | `Operation::build_request_plan` | returns [`RequestPlan`], whose `Debug` is `connector-resolve`'s redacting one — re-asserted below through the `connector_pack` path |
/// | `Egress::send` | takes a [`Request`](connector_pack::Request), whose `Debug` prints shape without values, and the `Egress`'s own `Debug` names the transport rather than the traffic |
/// | `RequestPlan` (re-export) | `redactions` is `Vec<SensitiveText>`; `permission_subjects` is the pre-authentication URL by construction |
#[tokio::test]
async fn nothing_the_plan_seam_publishes_prints_a_credential() {
    let recorder = Arc::new(Recorder::default());
    let (operation, egress) = projected(recorder).await;
    let ctx = context();

    let plan = operation
        .build_request_plan(&ctx, &params())
        .await
        .expect("the plan derives");
    let travelling = plan.redactions[0].expose_secret().to_string();

    for (surface, printed) in [
        ("the plan", format!("{plan:?}")),
        ("the plan's request", format!("{:?}", plan.request)),
        ("the plan's redaction set", format!("{:?}", plan.redactions)),
        ("the projected operation", format!("{operation:?}")),
        ("the bound egress", format!("{egress:?}")),
    ] {
        assert!(
            !printed.contains(SENTINEL),
            "{surface} prints the stored credential: {printed}"
        );
        assert!(
            !printed.contains(&travelling),
            "{surface} prints the credential as it travels: {printed}"
        );
    }

    // Shape without values, positively: the redaction is not "print nothing".
    let printed = format!("{:?}", plan.request);
    assert!(printed.contains("Authorization"), "{printed}");
    assert!(printed.contains("<redacted>"), "{printed}");

    // And a permission subject is safe to quote in an approval prompt, which is the one place
    // `Tool::permission_subjects` cannot consult a redactor.
    for subject in &plan.permission_subjects {
        assert!(!subject.contains(SENTINEL), "{subject}");
        assert!(!subject.contains(&travelling), "{subject}");
    }
}
