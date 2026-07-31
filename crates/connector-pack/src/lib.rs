//! The connector Tool pack: this repository's connectors, as flux tools.
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use std::sync::Arc;
//! # use connector_pack::{Credentials, Egress, SecretStore};
//! # use flux_runtime::{Tool, ToolRegistry};
//! # let configured_http_request_tool: Arc<dyn Tool> = flux_runtime::tool_fn(
//! #     flux_spec::ToolSpec { name: "http.request".into(), description: String::new(),
//! #         input_schema: serde_json::json!({}), output_schema: None, effects: Vec::new(),
//! #         risk: flux_spec::Risk::Medium, idempotency: flux_spec::Idempotency::NonIdempotent,
//! #         access: Vec::new(), group: None },
//! #     |params| async move { Ok(params) });
//! # let host_secret_store: Arc<dyn SecretStore> =
//! #     Arc::new(connector_pack::MemoryStore::new());
//! // flux's own `http.request`, already configured by the host — in a host that uses flux-web,
//! // `Arc::new(flux_web::http::HttpRequestTool::new(&options))`.
//! let http = Egress::new(configured_http_request_tool);
//! // Where this tenant's credentials live. Bound here, never looked up globally.
//! let credentials = Credentials::new(host_secret_store, "9f3a4b2c")?;
//!
//! let mut registry = ToolRegistry::new();
//! connector_pack::pack(&["zendesk"], http, credentials)(&mut registry)?;
//!
//! assert!(registry.get("zendesk.ticket.show").is_some());
//! # Ok(())
//! # }
//! ```
//!
//! or, from a host that uses flux's SDK, the same values straight into the builder:
//!
//! ```ignore
//! let http = Egress::new(Arc::new(flux_web::http::HttpRequestTool::new(&web_options)));
//! let credentials = Credentials::new(Arc::new(VaultStore::new(&vault)?), &tenant)?;
//! let client = flux_sdk::Client::builder()
//!     .try_register_pack(connector_pack::pack(&["zendesk", "slack"], http, credentials))
//!     .build()?;
//! ```
//!
//! # Why a Tool pack, and not more `.flux`
//!
//! `connectors/<provider>.flux` keeps shipping and keeps being the human-readable contract. This is
//! an **additional** surface, and it exists because of a naming asymmetry that no amount of
//! composite Flux can get around: a **dotted name is not a legal Flux declaration**, which is why
//! this repository emits `zendesk-ticket-show`, and **every flux tool is dotted** — `http.request`,
//! `op.register`, `skill.load`. flux's reference flow calls `zendesk.ticket.show`. It was written
//! against a tool surface, and only a tool surface can spell it. See [`dotted_name`].
//!
//! The safety argument is the stronger one. As a composite, an operation inherits whatever gating
//! `http.request` happens to get. As a Tool, each operation is gated **individually** by flux's
//! permission and approval envelope, at the risk level the connector author declared — a capability
//! the composite path cannot have.
//!
//! # This crate still ships no runtime
//!
//! It links flux's runtime types and constructs none of it. `vision.md`'s non-goal stands: *this
//! repo compiles; flux executes.* A host builds the registry, binds the ports and runs the loop;
//! [`pack`] hands it declarations. The compiler crates — `connector-spec`, `connector-flux`,
//! `connector-cli` — link none of this, and `connector-catalog` stays dependency-free.
//!
//! # flux keeps every byte of egress
//!
//! `Tool::execute` builds `{ method, url, headers, body }` and hands it to flux's own
//! `http.request`, passing the **same** `ctx`. This crate opens no socket, holds no HTTP client and
//! resolves no host: the transport is a constructor argument, so a host supplies the instance it has
//! already configured with its SSRF guard, its private-network grant and its audit sink.
//!
//! That delegation calls `http.request`'s `execute` directly, which **bypasses
//! `Executor::dispatch`** — so `http.request`'s own `permission_subjects` and `intents` are never
//! consulted for the inner call. Both have default trait implementations returning empty, so a Tool
//! that omitted them would compile, register, execute, reach the vendor, and never have the host's
//! network policy consulted. [`Operation`] declares both itself and `tests/network_gate.rs` holds
//! every shipped operation to it.
//!
//! # The credential never reaches a surface
//!
//! A credential is resolved here, assembled here — the `Bearer ` prefix, the basic-auth base64, the
//! query placement — and placed on a request here, which is what dissolves the `$auth` blocker:
//! flux's whole-value `{"$secret"}` marker never has to grow any of those capabilities. See
//! [`crate::auth`] for the three axes and [`Credentials`] for the port.
//!
//! The order is the safety property. Every value is registered with `ctx.redactor` **before the
//! request is constructed** — and before any fallible step that follows resolution — so a failure
//! between construction and dispatch cannot surface it. `flux-web`'s `http.rs:248` is the precedent,
//! and `tests/credentials.rs` holds it against all four surfaces `Executor::dispatch` scrubs.
//!
//! Registration is *checked*, not assumed, and that is the correction C-152 made. flux's
//! `Redactor::add_secret` silently ignores a value under six trimmed characters, so a short
//! credential was registered successfully and redacted nowhere, while this paragraph said otherwise.
//! A value the redactor does not end up holding is now [`Error::UnredactableCredential`] and **is not
//! sent** — the guarantee holds for every credential that travels, because one it cannot cover does
//! not travel. See `docs/designs/connector-tool-pack.md` for why refusing beat documenting the
//! threshold.
//!
//! # What is not here yet
//!
//! **Not every connector can be authenticated.** A credential's address is
//! `tenants/<tenant>/<authority>/<credential>`, and only two of the nineteen shipped connectors —
//! `slack` and `fly` — declare an `authority` (C-37). The rest refuse with
//! [`Error::NoCredentialAddress`] rather than sending an unauthenticated request — fail-closed, and
//! with a diagnostic naming the missing fact instead of a vendor's `401`.
//!
//! **No response shaping.** `http.request` returns one flat string
//! (`HTTP {status}\n{headers}\n{body}`), which is returned whole.
//!
//! **No config resolution**, so a templated base URL still carries `{subdomain}` verbatim. Five of
//! the nineteen connectors declare a templated host (`{subdomain}.zendesk.com`, `{domain}`,
//! `{site}.atlassian.net`, `{shop}.myshopify.com`), which is **27 of 105 operations that cannot
//! reach a vendor** until C-10's base-URL configuration lands. A credential does not change that,
//! and nothing here should be read as saying it does: an authenticated request to
//! `https://{subdomain}.zendesk.com/...` is still a request to a host that does not resolve.

mod auth;
mod credentials;
mod name;
mod request;
mod spec;
mod tool;

pub use credentials::{Credentials, DEFAULT_SERVICE};
pub use name::{dotted_name, NameError};
pub use request::Request;
pub use spec::project;
pub use tool::{Egress, Operation};

// The credential vocabulary, re-exported rather than redefined — the same posture
// `connector-secrets` takes towards `connector-spec`'s addressing. A host binding this pack's
// credential port should not have to name three crates to spell one address.
pub use connector_secrets::{
    CredentialRef, Layout, MemoryStore, Secret, SecretStore, StoreError, TenantLayout,
};

use catalog::ProviderKey;
use flux_runtime::{Tool, ToolRegistry};
use std::sync::Arc;

/// Why a pack could not be installed.
///
/// Every variant refuses; none repairs. A pack that installed *most* of a provider would leave a
/// host holding a connector that resolves some of its operations and silently not others.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The pack named a connector this catalogue does not carry.
    ///
    /// Reported rather than skipped: registering nothing and returning success is how a host ends
    /// up with a client that cannot call anything and no diagnostic saying why. A typo in a
    /// provider name is the overwhelmingly likely cause.
    #[error(
        "no connector named `{provider}` is in this catalogue — `catalog::providers()` lists the \
         {available} this build carries"
    )]
    UnknownProvider {
        /// The name that was asked for.
        provider: String,
        /// How many connectors the catalogue does carry.
        available: usize,
    },

    /// The operation's id has no dotted tool name. See [`NameError`].
    #[error("`{operation}` cannot be projected onto a flux tool name: {source}")]
    Name {
        /// The operation id.
        operation: String,
        /// Which end of the seam refused it.
        #[source]
        source: NameError,
    },

    /// The Flux embedded for an operation does not parse.
    ///
    /// Unreachable for a catalogue this repository generated —
    /// `crates/catalog/tests/embedded_operations.rs` parses every rendering on its own as a
    /// standing gate. It is reported as the corrupt-input case it would be rather than unwrapped,
    /// because the alternative is a panic inside a host's registration call.
    #[error("the Flux embedded for `{operation}` does not parse: {message}")]
    Unparsable {
        /// The operation id.
        operation: String,
        /// What flux-lang said.
        message: String,
    },

    /// A catalogue rendering is exactly one `op` declaration, and this one is not.
    #[error(
        "the Flux embedded for `{operation}` declares {found} operations; one catalogue rendering \
         is exactly one declaration"
    )]
    NotOneOperation {
        /// The operation id.
        operation: String,
        /// How many declarations were found.
        found: usize,
    },

    /// The projected spec is one flux itself will not register.
    ///
    /// flux pairs `effects` with `access`: a declared effect with no host capability that could
    /// carry it is refused by `authority_requirements_from_declaration`, and therefore by
    /// `try_register_from`. Unreachable for a declaration this repository emits — every generated
    /// op declares `["network"]` alone, which pairs with network access — and reported here rather
    /// than left to surface at a host's startup with only the tool name to go on.
    ///
    /// The refusal is deliberate. Satisfying flux by claiming an access kind the connector does not
    /// have would register a tool that looks gated and is not.
    #[error("`{operation}` projects onto a spec flux will not register: {message}")]
    Unregistrable {
        /// The operation id.
        operation: String,
        /// What flux's authority checker said.
        message: String,
    },

    /// A rendering declaring a different operation than the entry it is filed under.
    ///
    /// Left alone this would register a tool under a name derived from one id carrying a contract
    /// taken from another — the two halves of an operation coming apart with nothing to notice it.
    #[error(
        "the entry for `{operation}` embeds a declaration of `{declared}`; the catalogue's index \
         and its renderings disagree"
    )]
    Mismatched {
        /// The operation id the entry is filed under.
        operation: String,
        /// The operation the embedded Flux actually declares.
        declared: String,
    },

    /// An entry naming no host, which therefore cannot be network-gated.
    ///
    /// [`Operation::permission_subjects`](flux_runtime::Tool::permission_subjects) falls back to
    /// the declared hosts when a request cannot be built, so an entry with none would answer
    /// *empty* exactly when the answer matters most — and empty is the default the trait hands out
    /// for free, indistinguishable from a considered answer at every other layer. Unreachable for a
    /// catalogue this repository generated: `http_hosts` derives from a service's `base_url`
    /// (C-10), which is mandatory.
    #[error(
        "`{operation}` declares no host, so no permission subject could name where it goes; a \
         connector that cannot be network-gated does not install"
    )]
    NoDeclaredHost {
        /// The operation id.
        operation: String,
    },

    /// A call that omitted a parameter the operation declares.
    ///
    /// Refused rather than defaulted, because the failure it prevents is silent: an absent path
    /// parameter leaves its `{placeholder}` verbatim in the URL (flux's interpolator does not drop
    /// an unbound name), and the vendor answers that request. This is the same contract flux's own
    /// composite dispatch applies — every declared parameter is required, and an *optional* one is
    /// a parameter a caller may pass `null` for.
    #[error("`{operation}` was called without `{parameter}`, which it declares")]
    MissingParameter {
        /// The operation id.
        operation: String,
        /// The parameter that was not supplied.
        parameter: String,
    },

    /// An operation whose emitted body this pack cannot evaluate into a request.
    ///
    /// The refusal is the point. `connector-flux` emits one closed shape and
    /// [`crate::request`](crate) models exactly it; a body that grew a node beyond that — a quirk
    /// compiled into control flow (C-12), a `retry` — must fail here, because the alternative is a
    /// request assembled from *part* of an operation and sent anyway. A partly-evaluated request is
    /// not a degraded request, it is a different call, and the vendor answers it.
    #[error("`{operation}` cannot be built into a request: {message}")]
    Unbuildable {
        /// The operation id.
        operation: String,
        /// What the evaluator refused.
        message: String,
    },

    /// A tenant id that cannot be part of an address.
    ///
    /// Refused when the port is bound rather than at the first call: a tenant id is untrusted input
    /// that ends up in a store path, and the cautionary precedent is close to home — action-proxy
    /// puts two client-supplied headers straight into a Vault path with no validation at all.
    #[error("`{tenant}` cannot address a tenant's credentials: {reason}")]
    Tenant {
        /// The tenant id that was refused.
        tenant: String,
        /// `connector_spec::credential::validate_tenant`'s own explanation.
        reason: String,
    },

    /// **No credential is stored where the operation's credential lives.**
    ///
    /// The request is not sent. That is the whole point of the variant: an unauthenticated call is
    /// a fail-closed `401`, and a host treating `401` as retryable will loop against the vendor
    /// forever without ever being told what is actually missing.
    #[error(
        "`{operation}` needs a credential and none is stored at `{path}` — the request was not \
         sent ({alternatives} address(es) tried)"
    )]
    MissingCredential {
        /// The operation id.
        operation: String,
        /// The path the store looked at, **as the store's own layout renders it** — the place an
        /// operator has to go and put the value.
        path: String,
        /// How many alternative mechanisms were tried before giving up.
        alternatives: usize,
    },

    /// The secret store answered, and not with a value.
    ///
    /// Kept apart from [`MissingCredential`](Self::MissingCredential) deliberately: "unreachable"
    /// and "not configured" want opposite responses, and collapsing them is the gap C-91's error
    /// type exists to close. `StoreError` never carries a value, so this is safe to log.
    #[error("`{operation}` could not resolve `{credential}`: {source}")]
    CredentialStore {
        /// The operation id.
        operation: String,
        /// The credential that could not be resolved.
        credential: String,
        /// What the store said.
        #[source]
        source: connector_secrets::StoreError,
    },

    /// The connector declares no `authority`, so its credential has no address.
    ///
    /// C-37's `pid` is the second segment of every credential path, and a connector without one
    /// cannot say *where* its secrets live — so there is nothing to look up. Refused rather than
    /// defaulted: any invented authority would render a plausible path pointing at a value nobody
    /// ever stored, and the resulting `NotFound` would send an operator to the wrong place.
    #[error(
        "`{operation}` needs `{credential}`, but connector `{provider}` declares no `authority`, so \
         no credential address renders for it (C-37); the request was not sent"
    )]
    NoCredentialAddress {
        /// The operation id.
        operation: String,
        /// The connector that declares no authority.
        provider: String,
        /// The credential that therefore cannot be addressed.
        credential: String,
    },

    /// The address components do not compose into a valid [`CredentialRef`].
    ///
    /// Unreachable for a catalogue this repository generated — every component is validated at the
    /// loader — and reported as the corrupt-input case it would be rather than unwrapped.
    #[error("`{operation}` cannot address `{credential}`: {reason}")]
    CredentialAddress {
        /// The operation id.
        operation: String,
        /// The credential in question.
        credential: String,
        /// `CredentialRef::new`'s own explanation.
        reason: String,
    },

    /// An operation requiring a credential its connector does not declare.
    ///
    /// The loader refuses this, so it is a disagreement between the catalogue's operation table and
    /// its credential table rather than an authoring error — the same class of drift
    /// [`Mismatched`](Self::Mismatched) covers for renderings.
    #[error(
        "`{operation}` requires `{credential}`, which connector `{provider}` does not declare; the \
         catalogue's operations and its credentials disagree"
    )]
    UndeclaredCredential {
        /// The operation id.
        operation: String,
        /// The credential named by the operation.
        credential: String,
        /// The connector that does not declare it.
        provider: String,
    },

    /// An operation authenticating with a **signing** secret.
    ///
    /// Every other credential answers "where does this go on the way out"; a webhook signing secret
    /// has no answer, because it never goes out — it verifies bytes that arrived. Placing one on a
    /// request would hand the vendor the value that authenticates *their* calls inbound. The loader
    /// already refuses it (`AGENTS.md`'s authentication contract); this is the second lock.
    #[error(
        "`{operation}` authenticates with `{credential}`, which is an inbound signing secret \
             and never leaves"
    )]
    InboundCredential {
        /// The operation id.
        operation: String,
        /// The signing credential.
        credential: String,
    },

    /// A credential whose header the operation's own emitted Flux already sets.
    ///
    /// Refused rather than overwritten: a silent replacement would send a request that neither the
    /// module nor the pack describes, and the two surfaces would still look identical in isolation.
    #[error(
        "`{operation}` would place `{credential}` in `{header}`, which its own module already sets"
    )]
    CredentialCollision {
        /// The operation id.
        operation: String,
        /// The credential that could not be placed.
        credential: String,
        /// The header, as the module spells it.
        header: String,
    },

    /// A Basic credential whose **non-secret** user half is not configured.
    ///
    /// The user half is config — an email address, an account name — so it resolves from the
    /// declared environment variables rather than from the store. Refused rather than composed as
    /// `base64(":<secret>")`, which is a header the vendor answers with a `401` that says nothing
    /// about the missing variable.
    #[error(
        "`{operation}` needs the non-secret user half of `{credential}`, and none of `{env}` is set"
    )]
    MissingCredentialConfig {
        /// The operation id.
        operation: String,
        /// The credential whose user half is missing.
        credential: String,
        /// The environment-variable keys that were tried, in order.
        env: String,
    },

    /// **A credential the host's redactor will not hold, and therefore will not travel.**
    ///
    /// flux's `Redactor::add_secret` silently ignores a value under six characters once trimmed
    /// (`codewandler-flux-secret-1.0.1/src/lib.rs:195-201`) — refusing to over-redact a common word
    /// is the right trade for flux, and it means registration can *succeed* while protecting
    /// nothing. A credential that short would then travel unredacted through all four surfaces
    /// `Executor::dispatch` scrubs, with every line of code above it reading as though it were
    /// protected.
    ///
    /// Refused rather than sent, which is C-152's recorded decision (see
    /// `docs/designs/connector-tool-pack.md`): a credential the host cannot keep off a surface is
    /// one it should not put on the wire, and a five-character API token is a misconfiguration long
    /// before it is a credential. The alternative considered was to state the threshold wherever the
    /// guarantee is stated and accept it; the design says why this one won.
    ///
    /// The refusal names the address so an operator can go and replace the value, and it names
    /// neither the value nor its length — a length is a fingerprint, which is the same care
    /// `connector_secrets::Secret`'s `Debug` takes.
    #[error(
        "`{operation}` resolved `{credential}` for tenant `{tenant}` under `{authority}`, and the \
         host's redactor will not hold a value that short, so it could not be kept off a surface; \
         the request was not sent"
    )]
    UnredactableCredential {
        /// The operation id.
        operation: String,
        /// The credential that could not be protected.
        credential: String,
        /// The tenant whose credential it is — the address, minus the value.
        tenant: String,
        /// The provider's reverse-DNS authority, the second segment of that address.
        authority: String,
    },

    /// A mechanism naming no credentials at all.
    ///
    /// It would authenticate nothing while looking satisfied, which is the one failure shape that
    /// resembles success. The loader refuses a degenerate empty mechanism; this is the second lock.
    #[error(
        "`{operation}` offers a mechanism that names no credentials, so it authenticates nothing"
    )]
    EmptyMechanism {
        /// The operation id.
        operation: String,
    },
}

impl From<Error> for flux_core::Error {
    fn from(error: Error) -> Self {
        flux_core::Error::Config(error.to_string())
    }
}

/// **The pack.** Install every operation of each named provider into a host's registry.
///
/// The returned value is `FnOnce(&mut ToolRegistry) -> flux_core::Result<()>` — exactly what
/// `flux_sdk::ClientBuilder::try_register_pack` takes, and equally callable against a bare
/// [`ToolRegistry`]. There is deliberately no `flux-sdk` dependency here: the SDK is the host's
/// choice, not this crate's, and a pack that required it could not be used by a host that
/// assembles its registry directly.
///
/// Each provider installs under **its own source label** (`connector-pack:<provider>`) through
/// [`ToolRegistry::try_register_all_from`], which is atomic: if any of a provider's operations is
/// invalid or collides with something already registered, none of that provider lands and flux's
/// own duplicate diagnostic names both contributors. A host composing this pack with its own
/// built-ins gets that diagnostic instead of a silent override.
///
/// The provider names are copied at call time, so the returned closure is independent of the slice
/// it was built from.
///
/// # The transport is an argument
///
/// Every operation this pack installs delegates its egress to the one [`Egress`] handed in here.
/// Taking it rather than constructing it is what lets a host supply the tool it has already
/// configured — its egress allow-list, its private-network grant, its audit sink. Constructing one
/// here would silently give connectors a *different* network policy from the rest of the host. See
/// [`Egress`] for what a substitute must honour, and for the one thing its type cannot enforce.
///
/// # So is the credential port, and it is not optional
///
/// [`Credentials`] is the other bound port, and it is a required argument rather than an
/// `Option`. A pack that could be built without one would let a host install connectors that send
/// every request unauthenticated — a fail-closed `401` from the vendor, but one a host treating
/// `401` as retryable will loop on forever. Requiring it makes "I forgot to bind a store" a
/// compile error instead of a production symptom.
///
/// # Errors
///
/// The closure returns an error when a named provider is not in the catalogue, when an operation
/// cannot be projected, or when flux refuses the registration. Nothing is installed for a provider
/// whose install failed; providers listed before it are already in, because a partially-composed
/// registry is the caller's to discard.
pub fn pack(
    providers: &[&str],
    http: Egress,
    credentials: Credentials,
) -> impl FnOnce(&mut ToolRegistry) -> flux_core::Result<()> {
    let requested: Vec<String> = providers.iter().map(|name| (*name).to_string()).collect();

    move |registry: &mut ToolRegistry| {
        for provider in &requested {
            install(registry, provider, &http, &credentials)?;
        }
        Ok(())
    }
}

/// Install one provider's operations under one source label.
fn install(
    registry: &mut ToolRegistry,
    provider: &str,
    http: &Egress,
    credentials: &Credentials,
) -> flux_core::Result<()> {
    let entry =
        catalog::provider(ProviderKey::id(provider)).ok_or_else(|| Error::UnknownProvider {
            provider: provider.to_owned(),
            available: catalog::providers().len(),
        })?;

    let tools = entry
        .operations
        .iter()
        .map(|operation| {
            Ok(Arc::new(Operation::project(
                operation,
                http.clone(),
                credentials.clone(),
            )?) as Arc<dyn Tool>)
        })
        .collect::<Result<Vec<_>, Error>>()?;

    registry.try_register_all_from(source_label(entry.id), tools)
}

/// The auditable label a provider's operations are registered under.
///
/// Per-provider rather than one label for the whole pack, so a duplicate diagnostic names the
/// connector that contributed the colliding operation rather than "the connector pack".
fn source_label(provider: &str) -> String {
    format!("connector-pack:{provider}")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A bound credential port over an **empty** store.
    ///
    /// Every test in this crate that does not itself care about credentials wants exactly this: the
    /// port is bound, because the pack requires one, and it holds nothing, because a test asserting
    /// a *request shape* must not depend on a value being present. The tests that do care live in
    /// `tests/credentials.rs` and put a sentinel in a store of their own.
    pub(crate) fn empty_credentials() -> Credentials {
        Credentials::new(
            Arc::new(connector_secrets::MemoryStore::new()),
            "t-connector-pack",
        )
        .expect("a valid tenant id")
    }

    /// A stand-in for flux's `http.request`, for tests that need a transport but not a socket.
    ///
    /// A real [`flux_runtime::ToolContext`] needs a `flux_system::System` over a real workspace
    /// root, which unit tests in this crate do not build — so what they assert is
    /// [`Operation::build_request`], the request *before* it is sent, which is where the two
    /// mistakes that matter live. `execute` is driven end to end from `tests/credentials.rs`, where
    /// the `flux-system` dev-dependency makes a real context available.
    pub(crate) fn recording_http() -> Egress {
        Egress::new(flux_runtime::tool_fn(
            flux_spec::ToolSpec {
                name: "http.request".into(),
                description: "a stand-in that echoes the request it was handed".into(),
                input_schema: serde_json::json!({"type": "object"}),
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

    #[test]
    fn a_providers_operations_are_labelled_with_the_provider() {
        let mut registry = ToolRegistry::new();
        pack(&["zendesk"], recording_http(), empty_credentials())(&mut registry)
            .expect("zendesk installs");

        assert_eq!(
            registry.source("zendesk.ticket.show"),
            Some("connector-pack:zendesk")
        );
    }

    /// The closure must not borrow the slice it was built from, or a host could not build a pack
    /// from a temporary and hand it to a builder.
    #[test]
    fn the_pack_outlives_the_names_it_was_built_from() {
        let install = {
            let names = vec!["zendesk"];
            pack(&names, recording_http(), empty_credentials())
        };

        let mut registry = ToolRegistry::new();
        install(&mut registry).expect("zendesk installs");
        assert!(registry.get("zendesk.ticket.show").is_some());
    }

    /// Two providers, one registry, one call — the shape the design's example uses.
    #[test]
    fn several_providers_install_together() {
        let mut registry = ToolRegistry::new();
        pack(&["zendesk", "slack"], recording_http(), empty_credentials())(&mut registry)
            .expect("both install");

        assert!(registry.get("zendesk.ticket.show").is_some());
        assert!(registry.get("slack.chat.post.message").is_some());
        assert_eq!(
            registry.source("slack.chat.post.message"),
            Some("connector-pack:slack")
        );
    }

    #[test]
    fn an_unknown_provider_names_itself() {
        let mut registry = ToolRegistry::new();
        // See `connector-catalog`'s note on negative sentinels: `salesforce` used to sit here and
        // stopped being unknown the moment C-163 shipped it.
        const NO_SUCH_PROVIDER: &str = "no-such-vendor";

        let error = pack(&[NO_SUCH_PROVIDER], recording_http(), empty_credentials())(&mut registry)
            .expect_err("no such connector");

        assert!(error.to_string().contains(NO_SUCH_PROVIDER), "{error}");
        assert!(registry.names().is_empty());
    }

    /// **One transport for the whole pack.** Every operation delegates to the instance the host
    /// supplied, so a host that configured one egress policy does not find connectors quietly using
    /// another.
    #[test]
    fn every_operation_delegates_to_the_transport_the_host_supplied() {
        let http = recording_http();
        let entries = catalog::operations_of(ProviderKey::id("zendesk"));
        assert!(!entries.is_empty(), "zendesk carries operations");

        for entry in entries {
            let operation = Operation::project(entry, http.clone(), empty_credentials())
                .expect("the entry projects");
            assert!(
                Arc::ptr_eq(http.tool(), operation.egress().tool()),
                "`{}` holds a transport the host did not supply",
                entry.id
            );
        }
    }
}
