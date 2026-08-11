//! **Credential diversion** (C-136): a login returns a handle, and the token never enters the
//! session.
//!
//! # The danger, and why redaction is not the answer
//!
//! An operation's result becomes a value in the session: bound to a symbol, interpolatable, visible
//! to the model that called it, eligible for a log line and for an error message. That is what an
//! operation *is*. So a login modelled as a normal operation hands a bearer token to a language
//! model as a string, and the requirement is violated by the call's own success.
//!
//! The tempting fix is to redact it, and this repository already knows why that is insufficient.
//! Redaction is *string matching against values it was already told about* — `ctx.redactor` holds
//! what [`crate::Credentials`] resolved out of the host's store, before the request was built. A
//! token minted **by this very call** is, by construction, unknown to it until after the value has
//! arrived, and by then something has handled a response body containing it.
//!
//! C-430 established the other half, and it is the constraint this module is shaped by: **removing
//! the field from the published schema is strictly worse than withholding the operation.** Nothing
//! between the vendor and a model-visible symbol projects a response — `connector-flux` emits
//! `return $response` and the pack hands back what the transport produced — so deleting a location
//! from a `response_schema` removes the *disclosure* and leaves the *exposure*.
//!
//! # So: divert, never return
//!
//! The secret travels from the transport's answer straight into the bound
//! [`SecretStore`](connector_secrets::SecretStore), and what comes back is C-90's
//! [`CredentialRef`] rendered as an address:
//!
//! ```text
//! { "credential": "tenants/<tenant>/<authority>/<credential>" }
//! ```
//!
//! Downstream operations already work by reference — they name the credential in `credentials` and
//! [`crate::Credentials`] resolves it at request-assembly time — so **a caller can use a credential
//! it can never read**. There is no policy to configure and no redaction pattern to maintain,
//! because no code path returns the value.
//!
//! # Nothing derived from the vendor's answer leaves this module, on any path through it
//!
//! That is the sentence to check this file against, and it is stronger than "the success path
//! returns a handle". Read the scope note below before quoting it: it is a claim about the host
//! path, and the module path is closed by a refusal rather than covered by this guarantee. The story's second named test is the failure path: *a login that errors after
//! the token arrives must not surface it in the error*, which is exactly the case redaction cannot
//! cover. So [`divert`] consumes the transport's result and either mints — answering with the
//! handle — or refuses with [`Error::CredentialNotMinted`], which carries the operation, the
//! credential and at most the vendor's **HTTP status**. Not the body, not a parse error quoting the
//! bytes it choked on, and not the transport's own message: a `401` body is where a vendor puts an
//! explanation, and a `200` body carrying an error is where several vendors put the token anyway.
//!
//! The cost is stated rather than hidden: an operator debugging a failing login gets a status and
//! not the vendor's reason. The request is in the host's evidence log; the answer is withheld
//! because this is the one operation shape where the answer is the credential.
//!
//! # The scope of that sentence: **this is the host path, and it is the only path**
//!
//! Said precisely, because the unqualified version of it was wrong when this module was first
//! written and a reviewer was right to catch it. What is guaranteed here is the `connector-pack`
//! path — a host binding the transport, the credential store and the configuration port, and calling
//! a projected [`Operation`]. This repository has a **second** execution surface: the emitted
//! `connectors/<provider>.flux` module, which a flux runtime lifts and runs directly, and which
//! nothing in this crate is on.
//!
//! There is no diversion there and there cannot be. An emitted `op` ends `response =
//! http.request(…)` / `return response`, and Flux has no handle on the credential store — so a
//! module carrying a login would perform it and bind the raw token to a model-visible symbol, which
//! is what `AGENTS.md` § Authentication contract has forbidden since before this story
//! (*"must not … perform session login"*).
//!
//! **So the module path is closed rather than covered.** `connector_flux::Error::CredentialProducingOperation`
//! refuses to emit any operation declaring `produces_credential`, which means such a connector does
//! not build at all — the guarantee this module makes is not being asked to stretch over a surface
//! it cannot reach. Whether a credential-producing operation should exist as an *operation* is the
//! open question in `docs/stories/C-136-credential-diversion.md`; the diversion itself is
//! indifferent to how it is triggered.
//!
//! # The store is the port the host bound
//!
//! [`crate::Credentials::mint`] writes through the same `Arc<dyn SecretStore>`
//! [`crate::Credentials::resolve`] reads through — the one handed to [`crate::pack`] at
//! construction. There is no global, no `OnceLock` and no ambient default, so **an operation cannot
//! mint into a store the host did not supply**; a host that bound none has a login that refuses.

use connector_secrets::{CredentialRef, Layout, TenantLayout};
use flux_runtime::{ToolContext, ToolResult};
use serde_json::{json, Value};

use crate::tool::Operation;
use crate::Error;

/// **What makes an operation credential-producing**, read off the catalogue at projection time.
///
/// The declaration is the connector's: `produces_credential` in the operation's own
/// `[[operations]]` block, which the loader refuses unless it names a secret location and a declared
/// credential, unless the operation is non-idempotent, and unless the operation's own
/// `response_schema` keeps clear of the secret. It reaches the catalogue on the credential, as
/// [`catalog::Acquisition::Minted`] — see that variant for why it is carried there.
///
/// Derived once, at install, for the reason [`Operation`]'s spec is: an operation whose minting
/// declaration could not be read is a refusal `pack` returns, not a panic inside a host's
/// registration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Minting {
    /// The credential the minted value is stored as. Its `leaf` is the last segment of the address.
    pub(crate) credential: &'static catalog::Credential,
    /// Where the secret arrives in the vendor's response body — one plain JSON Pointer into the
    /// response value, resolved by [`serde_json::Value::pointer`].
    ///
    /// **No wildcard, and the loader is what guarantees it.** `credential_response`'s vocabulary
    /// admits `*` for every element of an array; `produces_credential` refuses it, because a mint
    /// stores one value at one address. So the resolver here needs no extension — and if a `*` ever
    /// reached this field it would resolve to nothing and the call would refuse, which is the
    /// fail-closed direction.
    pub(crate) from: &'static str,
}

/// **The credential `entry` mints**, or `None` for the overwhelming majority of operations.
///
/// The join is by operation id, from the credential side: a connector's `[[auth]]` table is short
/// and the answer is needed once per operation per install. The loader has already refused two
/// operations minting one credential, so the first match is the only match.
///
/// # Errors
///
/// [`Error::InboundCredential`] for a login declared to mint a **signing** secret. A verification
/// secret never leaves — it checks bytes that arrived — and it is vendor-provisioned rather than
/// minted, so an arrangement that claims otherwise is a connector that has confused its two
/// directions. Refused at install, where it is one host's startup failure, rather than at the first
/// login.
pub(crate) fn declared_by(
    entry: &'static catalog::Operation,
    provider: &'static catalog::Provider,
) -> Result<Option<Minting>, Error> {
    let found = provider
        .auth
        .iter()
        .find_map(|credential| match credential.acquire {
            catalog::Acquisition::Minted { by, from } if by == entry.id => {
                Some(Minting { credential, from })
            }
            _ => None,
        });

    if let Some(minting) = &found {
        if matches!(minting.credential.place, catalog::Placement::Inbound) {
            return Err(Error::InboundCredential {
                operation: entry.id.to_owned(),
                credential: minting.credential.name.to_owned(),
            });
        }
    }
    Ok(found)
}

/// **Divert the minted secret into the store and answer with its address.**
///
/// `carried` is whatever the transport produced for a call that has already been built and
/// authenticated — so a refusal *before* this point is one of the pack's own, names a missing
/// parameter or an unprovisioned credential, and is reported unchanged. Everything from here on
/// describes the vendor's answer, and is withheld.
///
/// # Errors
///
/// [`Error::CredentialNotMinted`] whenever the secret cannot be taken out of the answer — the
/// transport failed, the answer is not JSON, the location resolves to nothing, or it resolves to
/// something that is not a non-empty string. Every one of them is the same refusal carrying the
/// same three facts, because distinguishing them for a caller would mean describing the body.
/// Plus whatever [`crate::Credentials::mint`] refuses: a value the host's redactor would silently
/// decline to hold, a connector with no address to compose, and a store that could not be written
/// to. None of them quotes a value — [`connector_secrets::StoreError`] is documented as safe to
/// log, and this module's own errors carry no body.
pub(crate) async fn divert(
    operation: &Operation,
    minting: &Minting,
    ctx: &ToolContext,
    carried: flux_core::Result<ToolResult>,
) -> flux_core::Result<ToolResult> {
    let withheld = |status: Option<u64>| Error::CredentialNotMinted {
        operation: operation.entry().id.to_owned(),
        credential: minting.credential.name.to_owned(),
        status,
    };

    // The transport's own error is dropped rather than wrapped. It is the one string in this
    // function most likely to quote the response — a vendor error, a recorded fixture, a dry-run
    // renderer — and there is no shape of it this module can inspect and vouch for.
    let Ok(result) = carried else {
        return Err(withheld(None).into());
    };

    // The canonical `ToolResult::content` of flux's own `http.request` is the record
    // `{status, headers, body}`, JSON-encoded, with `body` parsed when the response is a JSON
    // object or array. The location is a pointer into the **response body**, as `response_schema`
    // describes it, so `body` is what it is resolved against. A substitute transport answering with
    // the bare body is served by the fallback, and a pointer that resolves against neither is a
    // refusal rather than a guess.
    let Ok(document) = serde_json::from_str::<Value>(&result.content) else {
        return Err(withheld(None).into());
    };
    let status = document.get("status").and_then(Value::as_u64);
    let body = document.get("body").unwrap_or(&document);

    let secret = body
        .pointer(minting.from)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let Some(secret) = secret else {
        return Err(withheld(status).into());
    };

    let reference = operation.mint(ctx, minting.credential, secret).await?;
    Ok(handle(&reference))
}

/// **The operation's whole output**: the address, and nothing else.
///
/// Rendered through [`TenantLayout`] rather than through whatever [`Layout`] the host's store was
/// configured with, and the difference is deliberate. This is the credential's **address** — C-90's
/// identifier, the thing a downstream operation and an operator both name — while a store path is
/// one deployment's spelling of where that address happens to live. Handing back the store's
/// spelling would publish a deployment detail as an operation's contract, and would make the same
/// login answer differently under two hosts.
///
/// The `view` is the same sentence a model should read, and it says what it says on purpose: the
/// point of this operation is that its result is *not* the credential.
fn handle(reference: &CredentialRef) -> ToolResult {
    let address = TenantLayout.render(reference);
    ToolResult::ok_view(
        json!({ HANDLE_FIELD: address }).to_string(),
        format!(
            "The credential was stored at {address}. Its value is not returned — name this address \
             where the credential is required."
        ),
    )
}

/// **The property name the handle is returned under**, and the same word
/// `connector_spec::CREDENTIAL_HANDLE_FIELD` puts in the operation's declared output.
///
/// Mirrored rather than imported: this crate's input is the catalogue, and it depends on neither
/// `connector-spec` nor the compiler (`AGENTS.md` § Ownership boundaries).
///
/// **And unlike [`crate::DEFAULT_SERVICE`]'s mirror, this one cannot be checked by a test that sees
/// both sides** — no crate in this workspace depends on `connector-spec` *and* on this one, and
/// `crates/connector-cli/tests/dependency_fence.rs` is what keeps it that way. So the guard is the
/// weaker one available: each side pins the literal against its own test
/// (`tests/credential_diversion.rs` here, `tests/produces_credential.rs` there), and the word is
/// written out in both rather than derived, so a change to either fails a test that names the other.
pub(crate) const HANDLE_FIELD: &str = "credential";

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use connector_secrets::{MemoryStore, Secret, SecretStore, StoreError};
    use flux_runtime::{RuntimeTurnContext, Tool, ToolProgress, ToolProgressSink};
    use flux_system::{System, Workspace};

    use super::*;
    use crate::config::{Configuration, MemoryConfig};
    use crate::{Credentials, Egress, DEFAULT_SERVICE};

    /// **The token the stubbed vendor mints.** An obvious non-credential carrying none of
    /// `flux_secret`'s known prefixes (`xoxb-`, `sk-`, `ghp_`, …), so a pass cannot come from flux's
    /// shape-based redaction instead of from the diversion under test.
    const MINTED: &str = "MINTED-NOT-A-REAL-SECRET-C136";

    /// The credential the login authenticates *with* — a client secret the tenant provisioned
    /// before the call. Distinct from [`MINTED`] so an assertion about the minted value cannot pass
    /// because the other one was handled.
    const CLIENT: &str = "CLIENT-NOT-A-REAL-SECRET-C136";

    /// The tenant every address below is rendered under.
    const TENANT: &str = "t-c136";

    /// The operation the doctored connector mints through.
    const LOGIN: &str = "slack-chat-post-message";

    /// A recording progress sink, so the progress surface is asserted end to end.
    #[derive(Default)]
    struct Progress(Mutex<Vec<String>>);

    impl ToolProgressSink for Progress {
        fn emit(&self, progress: ToolProgress) {
            self.0.lock().unwrap().push(progress.line);
        }
    }

    /// A `ToolContext` with a progress sink installed. The workspace root is this crate's own
    /// directory: `System` requires one that exists, and nothing here reaches the filesystem.
    fn context(progress: Arc<Progress>) -> ToolContext {
        let workspace = Workspace::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
        let mut ctx = ToolContext::new(Arc::new(System::new(workspace)));
        ctx.set_runtime_turn_context(RuntimeTurnContext::new().with_tool_progress_sink(progress));
        ctx
    }

    /// `tenants/t-c136/com.slack.api/bot_token` — where the client secret is provisioned.
    fn client_address() -> CredentialRef {
        CredentialRef::new(TENANT, "com.slack.api", DEFAULT_SERVICE, "bot_token")
            .expect("a valid address")
    }

    /// `tenants/t-c136/com.slack.api/session_token` — where the minted value must end up.
    fn minted_address() -> CredentialRef {
        CredentialRef::new(TENANT, "com.slack.api", DEFAULT_SERVICE, "session_token")
            .expect("a valid address")
    }

    /// An in-memory store holding the client secret and nothing else.
    async fn store() -> Arc<MemoryStore> {
        let store = MemoryStore::new();
        store
            .put(&client_address(), &Secret::new(CLIENT))
            .await
            .expect("an in-memory put cannot fail");
        Arc::new(store)
    }

    /// **The shipped slack connector with a minted session token declared beside its bot token.**
    ///
    /// `slack.bot_token` is untouched — it is what the login authenticates *with*, and it is the
    /// catalogue's own entry. `slack.session_token` is the credential [`LOGIN`] mints, declared
    /// through [`catalog::Acquisition::Minted`] exactly as `connector-cli`'s emitter writes it for a
    /// connector whose `[[operations]]` block states `produces_credential`.
    ///
    /// Doctored because **no shipped connector mints a credential**: the four operations v0.9.0 and
    /// v0.9.1 withheld are withheld under C-430's `credential_response`, and reinstating one is a
    /// change to a provider file. Everything else here is committed catalogue data, including the
    /// `com.slack.api` authority the addresses above are composed from.
    fn minting_slack() -> &'static catalog::Provider {
        with_auth(&[
            catalog::Credential {
                name: "slack.bot_token",
                leaf: "bot_token",
                acquire: catalog::Acquisition::Static,
                place: catalog::Placement::Header {
                    name: "Authorization",
                    prefix: "Bearer ",
                },
                subject: catalog::Subject::App,
            },
            catalog::Credential {
                name: "slack.session_token",
                leaf: "session_token",
                acquire: catalog::Acquisition::Minted {
                    by: LOGIN,
                    from: "/access_token",
                },
                place: catalog::Placement::Header {
                    name: "Authorization",
                    prefix: "Bearer ",
                },
                subject: catalog::Subject::App,
            },
        ])
    }

    /// The shipped slack connector with `auth` replaced.
    fn with_auth(credentials: &[catalog::Credential]) -> &'static catalog::Provider {
        let mut provider = *catalog::provider(catalog::ProviderKey::id("slack"))
            .expect("the shipped catalogue carries slack");
        provider.auth = Box::leak(credentials.to_vec().into_boxed_slice());
        Box::leak(Box::new(provider))
    }

    fn stand_in_spec() -> flux_spec::ToolSpec {
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in vendor that mints a token".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        }
    }

    /// **The stubbed vendor**: a login answering with a freshly minted token, in flux-web's
    /// canonical `{status, headers, body}` record and with a model-facing `view` beside it.
    ///
    /// The `view` carries the token too, deliberately. A stand-in that put it only in `content`
    /// would let a `view` assertion pass for the wrong reason, which is the mistake C-152 found in
    /// `tests/credentials.rs`.
    struct Vendor;

    #[async_trait]
    impl Tool for Vendor {
        fn spec(&self) -> flux_spec::ToolSpec {
            stand_in_spec()
        }

        async fn execute(
            &self,
            _ctx: &ToolContext,
            _params: Value,
        ) -> flux_core::Result<ToolResult> {
            let body = json!({
                "access_token": MINTED,
                "token_type": "Bearer",
                "expires_in": 3600,
            });
            Ok(ToolResult::ok_view(
                json!({ "status": 200, "headers": {}, "body": body }).to_string(),
                format!("HTTP 200\n\n{body}"),
            ))
        }
    }

    /// **A transport that fails after reading the answer**, quoting it — a recorded fixture, a proxy
    /// folding a body into its message, a vendor error echoing what it sent.
    struct FailingVendor;

    #[async_trait]
    impl Tool for FailingVendor {
        fn spec(&self) -> flux_spec::ToolSpec {
            stand_in_spec()
        }

        async fn execute(
            &self,
            _ctx: &ToolContext,
            _params: Value,
        ) -> flux_core::Result<ToolResult> {
            Err(flux_core::Error::Config(format!(
                "the upstream proxy failed after reading the response: \
                 {{\"access_token\": \"{MINTED}\"}}"
            )))
        }
    }

    /// **A vendor refusing the login**, and answering with a body that still carries a token.
    ///
    /// Not a contrivance: several vendors answer a failed grant with `200` and a body carrying a
    /// token for a different scope, and a `401` body is where the rest put an explanation. Returning
    /// either is the same exposure through a different door.
    struct RefusingVendor;

    #[async_trait]
    impl Tool for RefusingVendor {
        fn spec(&self) -> flux_spec::ToolSpec {
            stand_in_spec()
        }

        async fn execute(
            &self,
            _ctx: &ToolContext,
            _params: Value,
        ) -> flux_core::Result<ToolResult> {
            let body = json!({ "error": "invalid_client", "other_token": MINTED });
            Ok(ToolResult::ok_view(
                json!({ "status": 401, "headers": {}, "body": body }).to_string(),
                format!("HTTP 401\n\n{body}"),
            ))
        }
    }

    /// **A store that answers reads and refuses writes.** The sharpest reading of "a login that
    /// errors *after* the token arrives": the value is in this process, the vendor is finished, and
    /// the diversion is the step that fails.
    struct RefusingStore(Arc<MemoryStore>);

    #[async_trait]
    impl SecretStore for RefusingStore {
        async fn get(&self, reference: &CredentialRef) -> Result<Secret, StoreError> {
            self.0.get(reference).await
        }

        async fn put(
            &self,
            _reference: &CredentialRef,
            _secret: &Secret,
        ) -> Result<(), StoreError> {
            Err(StoreError::Unreachable {
                path: "a store that is deliberately down".to_owned(),
                reason: "the vault is sealed".to_owned(),
            })
        }

        async fn delete(&self, reference: &CredentialRef) -> Result<(), StoreError> {
            self.0.delete(reference).await
        }
    }

    /// Slack's `chat.postMessage` parameters. The login is that operation doctored, so it takes
    /// what that operation declares — a grant's own parameters are a provider-file matter.
    fn login_params() -> Value {
        json!({ "channel": "C0FLUX", "text": "hello", "thread_ts": null })
    }

    /// The login, projected against `vendor` and `store`.
    fn login(vendor: Arc<dyn Tool>, store: Arc<dyn SecretStore>) -> Operation {
        projected(minting_slack(), vendor, store).expect("the login projects")
    }

    fn projected(
        provider: &'static catalog::Provider,
        vendor: Arc<dyn Tool>,
        store: Arc<dyn SecretStore>,
    ) -> Result<Operation, Error> {
        let entry = catalog::operation(catalog::OperationKey::id(LOGIN))
            .expect("the shipped catalogue carries the operation the login is doctored from");
        Operation::project_onto(
            entry,
            provider,
            Egress::new(vendor),
            Credentials::new(store, TENANT).expect("a valid tenant"),
            Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant"),
        )
    }

    /// **The story's named test.** A credential minted by this very call reaches no surface at all.
    ///
    /// Every assertion is made against the **unredacted** value, and that is the point rather than
    /// an oversight. Redaction is string matching against values it was already told about; a token
    /// minted by this call is unknown to `ctx.redactor` until after it has arrived. A test that
    /// scrubbed first would pass against an implementation that returns the vendor's body and hopes,
    /// which is the implementation this story replaces.
    #[tokio::test]
    async fn a_minted_credential_never_reaches_the_session() {
        let progress = Arc::new(Progress::default());
        let ctx = context(progress.clone());
        let store = store().await;

        let tool = login(Arc::new(Vendor), store.clone());
        let result = tool
            .execute(&ctx, login_params())
            .await
            .expect("the stubbed vendor answers with a token");

        // **The control.** Without it every assertion below would pass against a login that never
        // reached the vendor, or against one whose declared location matched nothing.
        let stored = store
            .get(&minted_address())
            .await
            .expect("the minted value must have been diverted into the bound store");
        assert_eq!(
            stored.expose_secret(),
            MINTED,
            "the diversion stored something other than the value the vendor minted"
        );

        // Surface 1 — the operation's result value.
        assert!(
            !result.content.contains(MINTED),
            "the minted credential reached the operation's result value: {}",
            result.content
        );
        // And what it *does* carry is the handle: the address, and nothing else.
        let handle: Value = serde_json::from_str(&result.content).expect("the handle is JSON");
        assert_eq!(
            handle[HANDLE_FIELD],
            json!("tenants/t-c136/com.slack.api/session_token"),
            "the result is not the credential's address: {handle}"
        );
        assert_eq!(
            handle.as_object().map(serde_json::Map::len),
            Some(1),
            "the handle carries more than the address: {handle}"
        );

        // Surface 2 — the model-facing view, which the stand-in vendor deliberately fills with the
        // token, so an implementation passing the vendor's view through fails here alone.
        let view = result
            .view
            .as_deref()
            .expect("the handle carries a view, or this assertion is vacuous");
        assert!(
            !view.contains(MINTED),
            "the minted credential reached the model-facing view: {view}"
        );

        // Surface 4 — a progress line. The value is registered with the redactor as the second line
        // of defence the story asks for, so even a line a host writes about the call is covered.
        ctx.progress_reporter("slack.chat.post.message")
            .expect("a sink is installed")
            .report(&format!("minted {MINTED}"));
        let lines = progress.0.lock().unwrap().clone();
        assert!(!lines.is_empty(), "the sink recorded nothing");
        for line in lines {
            assert!(
                !ctx.redactor.redact(&line).contains(MINTED),
                "the minted credential survived into a progress line: {line}"
            );
        }
    }

    /// **Surface 3, and the case the story exists for**: a login that errors *after* the token
    /// arrived must not surface it in the error.
    ///
    /// Two failures, because they fail at different moments. The transport failing with the answer
    /// in its message is the ordinary shape; the store refusing the write is the sharp one — the
    /// value is in this process, the vendor is finished, and there was nothing a redactor could have
    /// been told in time by any mechanism that waits for a value to be resolved.
    #[tokio::test]
    async fn a_login_that_fails_after_the_token_arrives_does_not_surface_it() {
        let ctx = context(Arc::new(Progress::default()));

        let quoting = login(Arc::new(FailingVendor), store().await);
        let error = quoting
            .execute(&ctx, login_params())
            .await
            .expect_err("a login that minted nothing is a refusal, not a result");
        assert!(
            !error.to_string().contains(MINTED),
            "the minted credential survived into the error a failed login raises: {error}"
        );
        assert!(
            error.to_string().contains(LOGIN) && error.to_string().contains("slack.session_token"),
            "the refusal must still say which call was to mint what: {error}"
        );

        let refusing = login(
            Arc::new(Vendor),
            Arc::new(RefusingStore(store().await)) as Arc<dyn SecretStore>,
        );
        let error = refusing
            .execute(&ctx, login_params())
            .await
            .expect_err("a mint that cannot be stored is a refusal");
        assert!(
            !error.to_string().contains(MINTED),
            "the minted credential survived into the error a failed diversion raises: {error}"
        );
        // Unredacted above, and held by the redactor as well: `Credentials::mint` registers the
        // value before the store write, which is the only fallible step after it.
        assert_ne!(
            ctx.redactor.redact(MINTED),
            MINTED,
            "the value was in this process and the redactor had never been told about it"
        );
    }

    /// **A vendor answer carrying no credential is withheld too**, and the refusal carries the
    /// status rather than the body.
    #[tokio::test]
    async fn a_login_that_mints_nothing_returns_no_vendor_body() {
        let ctx = context(Arc::new(Progress::default()));
        let tool = login(Arc::new(RefusingVendor), store().await);
        let error = tool
            .execute(&ctx, login_params())
            .await
            .expect_err("a login that minted nothing is a refusal, not a result");

        assert!(
            !error.to_string().contains(MINTED),
            "the vendor's body reached the caller through the failure path: {error}"
        );
        assert!(
            !error.to_string().contains("invalid_client"),
            "the vendor's body reached the caller through the failure path: {error}"
        );
        assert!(
            error.to_string().contains("401"),
            "the refusal withholds the body and should still carry the status: {error}"
        );
    }

    /// **The store is the port the host bound, never a global.**
    ///
    /// Asserted as the property that can actually be observed — *this* store received it and one the
    /// host did not hand over did not — because "there is no global" is not a thing a test can see.
    #[tokio::test]
    async fn a_login_mints_only_into_the_store_the_host_bound() {
        let ctx = context(Arc::new(Progress::default()));
        let bound = store().await;
        let other = store().await;

        login(Arc::new(Vendor), bound.clone())
            .execute(&ctx, login_params())
            .await
            .expect("the stubbed vendor answers with a token");

        assert!(
            bound.get(&minted_address()).await.is_ok(),
            "the bound store did not receive the minted value"
        );
        assert!(
            matches!(
                other.get(&minted_address()).await,
                Err(StoreError::NotFound { .. })
            ),
            "a store the host did not hand this operation received the minted value"
        );
    }

    /// **A login declared to mint a signing secret is refused at install.**
    ///
    /// A verification secret never leaves — it checks bytes that arrived — and it is
    /// vendor-provisioned rather than minted, so an arrangement claiming otherwise is a connector
    /// that has confused its two directions.
    #[tokio::test]
    async fn a_login_minting_an_inbound_secret_is_refused_at_install() {
        let provider = with_auth(&[catalog::Credential {
            name: "slack.signing_secret",
            leaf: "signing_secret",
            acquire: catalog::Acquisition::Minted {
                by: LOGIN,
                from: "/access_token",
            },
            place: catalog::Placement::Inbound,
            subject: catalog::Subject::App,
        }]);

        let error = projected(provider, Arc::new(Vendor), store().await)
            .expect_err("a login cannot mint a secret that never leaves");
        assert!(matches!(error, Error::InboundCredential { .. }), "{error}");
    }

    /// **An operation that mints nothing is unchanged**, which is every operation the catalogue
    /// ships. The control for the whole file: without it, "the vendor body never comes back" could
    /// be true because it never comes back for anything.
    #[tokio::test]
    async fn an_ordinary_operation_still_returns_what_the_transport_produced() {
        let ctx = context(Arc::new(Progress::default()));
        let provider = with_auth(&[catalog::Credential {
            name: "slack.bot_token",
            leaf: "bot_token",
            acquire: catalog::Acquisition::Static,
            place: catalog::Placement::Header {
                name: "Authorization",
                prefix: "Bearer ",
            },
            subject: catalog::Subject::App,
        }]);

        let result = projected(provider, Arc::new(Vendor), store().await)
            .expect("an ordinary operation projects")
            .execute(&ctx, login_params())
            .await
            .expect("the stubbed vendor answers");
        assert!(
            result.content.contains("access_token"),
            "an operation that mints nothing must hand back what the transport produced: {}",
            result.content
        );
    }

    /// **The word the handle is returned under**, pinned here because no test can see both this
    /// crate and `connector-spec` — see [`HANDLE_FIELD`]. The counterpart is
    /// `crates/connector-spec/tests/produces_credential.rs`.
    #[test]
    fn the_handle_field_is_the_word_the_declared_output_uses() {
        assert_eq!(HANDLE_FIELD, "credential");
    }
}
