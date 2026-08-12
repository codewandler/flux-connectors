//! Why a request plan could not be derived.
//!
//! Every variant refuses; none repairs. They are the subset of `connector_pack::Error` that the
//! **plan derivation** raises, carried here because this crate is where the derivation now lives.
//! C-557 widened that derivation to the two **producers** — the endpoint resolver and the credential
//! assembler — so the config refusals (`MissingConfig`, `UnapprovedConfig`, `UnsafeOrigin`) and the
//! credential refusals (`MissingCredential`, `CredentialStore`, `NoCredentialAddress`,
//! `CredentialAddress`, `UndeclaredCredential`, `MissingCredentialConfig`, `EmptyMechanism`) join
//! the ones the template evaluator already raised, each still twinned with `connector_pack::Error`.
//!
//! # Two spellings, held together by a test rather than by care
//!
//! `connector_pack::Error` keeps every one of its variants — the name, the fields and the sentence
//! (C-538's acceptance says so in as many words, because a host matches on them). So each variant
//! below has a twin there, and `connector_pack`'s `From<connector_resolve::Error>` maps one onto the
//! other. That is two copies of seven sentences, which is a cost worth naming: it is paid because
//! collapsing them would either move `connector_pack::Error` into this crate — dragging
//! `NameError`, and with it `flux_lang`'s identifier predicates, across the engine fence this crate
//! exists to hold — or make the pack's variants unreachable, which is the thing the acceptance
//! forbids.
//!
//! The duplication is *pinned*, not trusted:
//! `connector_pack`'s `the_mapped_refusals_render_the_same_sentence` renders both sides of every
//! mapped variant and requires the strings to be equal, so a reworded refusal here fails there in
//! the same run.

/// Why a request plan could not be derived from the document.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A call that omitted a parameter the operation declares.
    ///
    /// Refused rather than defaulted: an absent path parameter leaves its `{placeholder}` verbatim
    /// in the URL, and the vendor answers that request.
    #[error("`{operation}` was called without `{parameter}`, which it declares")]
    MissingParameter {
        /// The operation id.
        operation: String,
        /// The parameter that was not supplied.
        parameter: String,
    },

    /// A caller-supplied path parameter that would leave its reviewed URL segment (C-478).
    #[error(
        "`{operation}` cannot place caller parameter `{parameter}` in one path segment: {reason}; \
         the request was not sent"
    )]
    UnsafePathParameter {
        /// The operation id.
        operation: String,
        /// The caller-visible parameter the request template places in the path.
        parameter: String,
        /// Why its value would escape or reshape one segment.
        reason: String,
    },

    /// An operation whose request template this crate cannot evaluate into a request.
    ///
    /// The refusal is the point. The template vocabulary is closed and total; a document that grew
    /// a spelling beyond it must fail here, because the alternative is a request assembled from
    /// *part* of an operation and sent anyway.
    #[error("`{operation}` cannot be built into a request: {message}")]
    Unbuildable {
        /// The operation id.
        operation: String,
        /// What the evaluator refused.
        message: String,
    },

    /// **A configuration value that would reshape the request it is substituted into** (C-214).
    #[error(
        "`{operation}` cannot substitute the configured `{variable}` into the {position} of its \
         request: {reason}; the request was not sent"
    )]
    UnsafeConfig {
        /// The operation id.
        operation: String,
        /// The configuration variable whose value was refused.
        variable: String,
        /// Where the value would have landed.
        position: &'static str,
        /// What is wrong with the value.
        reason: String,
    },

    /// A finished URL still naming a configuration variable.
    #[error(
        "`{operation}` built the URL `{url}`, which still names the configuration variable \
         `{variable}`; a parameter value put it there, and the request was not sent"
    )]
    UnresolvedEndpoint {
        /// The operation id.
        operation: String,
        /// The variable still named in the URL.
        variable: String,
        /// The URL as it was built. Unauthenticated — no credential has been placed on it yet.
        url: String,
    },

    /// A credential whose header the operation's own request template already sets.
    #[error(
        "`{operation}` would place `{credential}` in `{header}`, which its own module already sets"
    )]
    CredentialCollision {
        /// The operation id.
        operation: String,
        /// The credential that could not be placed.
        credential: String,
        /// The header, as the template spells it.
        header: String,
    },

    /// An operation authenticating with a **signing** secret, which never leaves.
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

    // -- the endpoint resolver's refusals (C-557) ------------------------------------------------
    /// **A connection setting the tenant has not supplied**, so the request cannot be composed.
    #[error(
        "`{operation}` needs `{field}` of service `{service}` of connector `{provider}` for tenant \
         `{tenant}`, and the bound configuration supplies none, so no URL composes; the request \
         was not sent"
    )]
    MissingConfig {
        /// The operation id.
        operation: String,
        /// The connector whose configuration is incomplete.
        provider: String,
        /// The connector's service the value was looked up under.
        service: String,
        /// The tenant the value was looked up for.
        tenant: String,
        /// The missing field, as `binds` spells it.
        field: String,
    },

    /// A connection-level value whose declaration requires deployment/operator approval has not been
    /// approved. The value itself is intentionally absent from this diagnostic.
    #[error(
        "`{operation}` cannot activate configuration field `{field}` of service `{service}` of \
         connector `{provider}` until deployment/operator policy approves and pins it; the request \
         was not sent"
    )]
    UnapprovedConfig {
        /// The operation id.
        operation: String,
        /// The connector the field belongs to.
        provider: String,
        /// The service the field belongs to.
        service: String,
        /// The configuration field, as `binds` spells it.
        field: String,
    },

    /// A configured origin did not satisfy the declared HTTPS-origin grammar. The value is never
    /// quoted into the refusal, logs or evidence.
    #[error(
        "`{operation}` cannot activate origin field `{field}` of service `{service}` of connector \
         `{provider}`: {reason}; the request was not sent"
    )]
    UnsafeOrigin {
        /// The operation id.
        operation: String,
        /// The connector the field belongs to.
        provider: String,
        /// The service the field belongs to.
        service: String,
        /// The origin field, as its declaration names it.
        field: String,
        /// Why the value is not a canonical HTTPS origin.
        reason: String,
    },

    // -- the credential assembler's refusals (C-557) ---------------------------------------------
    /// **No credential is stored where the operation's credential lives.** The request is not sent.
    #[error(
        "`{operation}` needs a credential and none is stored at `{path}` — the request was not \
         sent ({alternatives} address(es) tried)"
    )]
    MissingCredential {
        /// The operation id.
        operation: String,
        /// The path the store looked at, as the store's own layout renders it.
        path: String,
        /// How many alternative mechanisms were tried before giving up.
        alternatives: usize,
    },

    /// The secret store answered, and not with a value.
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

    /// The address components do not compose into a valid `CredentialRef`.
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

    /// A Basic credential whose **non-secret** user half is not configured.
    #[error(
        "`{operation}` needs the non-secret user half of `{credential}` for tenant `{tenant}`, and \
         the bound configuration supplies none (elsewhere this value is called `{env}`); the \
         request was not sent"
    )]
    MissingCredentialConfig {
        /// The operation id.
        operation: String,
        /// The credential whose user half is missing.
        credential: String,
        /// The tenant it was looked up for.
        tenant: String,
        /// What the same value is called in the vendor's documentation and in flux's `AuthMethod`.
        env: String,
    },

    /// A mechanism naming no credentials at all.
    #[error(
        "`{operation}` offers a mechanism that names no credentials, so it authenticates nothing"
    )]
    EmptyMechanism {
        /// The operation id.
        operation: String,
    },

    // -- the channel-handshake producer's refusals (C-558) ---------------------------------------
    /// A binding whose transport is not `socket` cannot be composed as a WebSocket handshake.
    #[error("connector `{provider}` binding `{binding}` is not a socket channel")]
    NotSocketChannel {
        /// The connector the binding belongs to.
        provider: String,
        /// The channel binding that is not a socket.
        binding: String,
    },

    /// A vendor-specific socket carries no generic RFC 6455 `connect` declaration to compose from.
    #[error(
        "connector `{provider}` binding `{binding}` is vendor-specific and has no generic connect \
         plan"
    )]
    VendorSocketChannel {
        /// The connector the binding belongs to.
        provider: String,
        /// The vendor-specific socket binding.
        binding: String,
    },
}
