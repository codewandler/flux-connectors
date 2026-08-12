//! Why a request plan could not be derived.
//!
//! Every variant refuses; none repairs. They are the subset of `connector_pack::Error` that the
//! **plan derivation** raises, carried here because this crate is where the derivation now lives.
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
}
