//! The one walk the manifest and the public catalogue share for a connector's **inbound** surface
//! (C-83).
//!
//! Events and channel bindings have been in the IR and in the hash domain since C-82 and reached no
//! artifact at all, so a host had no way to read what a connector declares. Two backends publish
//! them now — [`crate::seam`]'s `.connector.toml` and [`crate::site`]'s `catalog.json` — and the
//! encodings differ deliberately: TOML has no `null`, so the manifest omits an absent key, while the
//! published document holds every key always present.
//!
//! What must **not** differ is the judgement underneath, which is why it lives here rather than in
//! either backend. `crates/connector-cli/src/catalog.rs` and `src/site.rs` already share the
//! credential and host walks so that a site and a `cargo add` consumer cannot be told different
//! things about one operation; this is the same rule applied to the other call direction.
//!
//! # The judgement: how loudly an ingress surface says it is unverified
//!
//! `ChannelBinding::verification` is a tri-state, and the third state is the one that matters. An
//! *unset* verification is legal for a socket or a poll and a loader error for a webhook; an
//! explicit `verification = "none"` is an author saying the vendor publishes no signature at all.
//! Publishing that by *omitting* a key would put the two indistinguishable cases — "nothing arrives
//! unsolicited here" and "anyone can POST to this and we cannot prove otherwise" — behind the same
//! absent field.
//!
//! So the projection is **total**: every published binding carries a verification block, that block
//! always names its [`kind`](Verification::kind), and it restates the one boolean a consumer
//! actually filters on. That is the same shape `status.works` already has in the published document,
//! for the same reason: a consumer decides without knowing the vocabulary.

use connector_spec::{
    ChannelBinding, Digest, Encoding, FieldSource, HmacSpec, TimestampFormat, Transport,
    VerificationScheme,
};

/// How an inbound request on one binding proves it came from the vendor — the published,
/// **total** form of `ChannelBinding::verification`'s tri-state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verification {
    /// An HMAC over the raw request bytes, whose parameters the binding carries.
    Hmac,
    /// **Deliberately unverifiable.** The author stated `verification = "none"`: the vendor
    /// publishes no signature, so an endpoint accepts whatever arrives. AGENTS.md's rule is that
    /// silence is never a verification answer, and this is the value that keeps the statement loud
    /// instead of letting it read as an omission.
    None,
    /// Authenticated by the connection itself, not by a signature.
    ///
    /// A socket is opened *outward* by flux and authenticated by the credential that opened it, and
    /// a poll is an ordinary outbound call — nothing arrives unsolicited on either, so neither has
    /// a signature to check and neither states one. This is the unset arm of the tri-state, and it
    /// is a positive answer rather than the absence of one.
    Connection,
}

impl Verification {
    /// How this binding's verification is published — `hmac`, `none` or `connection`.
    ///
    /// A stable machine token, the same way an issue `code` is: a consumer switches on it without
    /// reading this crate.
    pub(crate) fn kind(self) -> &'static str {
        match self {
            Self::Hmac => "hmac",
            Self::None => "none",
            Self::Connection => "connection",
        }
    }

    /// Whether a delivery on this binding can be attributed to the vendor at all.
    ///
    /// Exactly `kind != "none"`, restated so a consumer filters on one boolean without knowing the
    /// vocabulary above — the same restatement `status.works` makes of `issues.is_empty()`. It is
    /// the field that makes a deliberately-unverifiable surface tell itself apart from a verified
    /// one **without anyone inspecting the absence of a key**.
    pub(crate) fn verified(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Classify one binding's verification.
///
/// The unset arm reads the transport, and it can do so honestly because the loader has already
/// refused the one combination that would make it a lie: a `webhook` that states nothing never
/// reaches here.
pub(crate) fn verification_of(channel: &ChannelBinding) -> Verification {
    match (&channel.verification, channel.transport) {
        (Some(VerificationScheme::Hmac(_)), _) => Verification::Hmac,
        (Some(VerificationScheme::None), _) => Verification::None,
        // Unset. Legal only for the two transports nothing arrives unsolicited on; a webhook is a
        // loader error, so this arm cannot silently launder one.
        (None, Transport::Socket | Transport::Poll) => Verification::Connection,
        (None, Transport::Webhook) => Verification::None,
    }
}

/// The token one transport is published under — the IR's own `snake_case` encoding, not a second
/// spelling of it.
///
/// Exhaustive on purpose, for the reason every other match in this crate is: a transport added to
/// the IR is a compile error here rather than a binding published with a missing field.
pub(crate) fn transport_token(transport: Transport) -> &'static str {
    match transport {
        Transport::Webhook => "webhook",
        Transport::Socket => "socket",
        Transport::Poll => "poll",
    }
}

/// The token one HMAC digest is published under. Exhaustive for the same reason.
pub(crate) fn digest_token(digest: Digest) -> &'static str {
    match digest {
        Digest::Sha1 => "sha1",
        Digest::Sha256 => "sha256",
    }
}

/// The token one signature encoding is published under.
pub(crate) fn encoding_token(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Hex => "hex",
        Encoding::Base64 => "base64",
    }
}

/// The token one timestamp spelling is published under.
pub(crate) fn timestamp_format_token(format: TimestampFormat) -> &'static str {
    match format {
        TimestampFormat::UnixSeconds => "unix_seconds",
        TimestampFormat::Rfc3339 => "rfc3339",
    }
}

/// How a binding's signed `{timestamp}` is spelled, as the artifacts publish it — or `None` for a
/// scheme that interpolates no timestamp, which has no spelling to state.
///
/// **The default is resolved here rather than passed on.** `HmacSpec::timestamp_format` is optional
/// in the IR because an author who writes nothing means unix seconds; a host reading an artifact must
/// not be asked to know that, because the cost of guessing the spelling of a signed timestamp is a
/// refused delivery at best. This is the same resolution `connector-spec`'s reference verifier makes.
///
/// The guard is the *selector*, not the format: the loader accepts a `timestamp_format` exactly when
/// `signed` interpolates `{timestamp}`, which is exactly when it accepts a `timestamp` selector, so
/// reading one answers for both.
pub(crate) fn timestamp_format_of(spec: &HmacSpec) -> Option<&'static str> {
    spec.timestamp
        .as_ref()
        .map(|_| timestamp_format_token(spec.timestamp_format.unwrap_or_default()))
}

/// The token one selector's source is published under — where on the inbound request a value is
/// read from.
pub(crate) fn source_token(source: FieldSource) -> &'static str {
    match source {
        FieldSource::Header => "header",
        FieldSource::Body => "body",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use connector_spec::{Digest, Encoding, HmacSpec};

    fn channel(transport: Transport, verification: Option<VerificationScheme>) -> ChannelBinding {
        ChannelBinding {
            name: "hook".to_string(),
            service: connector_spec::DEFAULT_SERVICE.to_string(),
            description: String::new(),
            transport,
            connect: None,
            events: Vec::new(),
            verification,
            discriminator: None,
            delivery_id: None,
            payload: Default::default(),
            payload_root: false,
            reply: None,
            cursor: None,
            subscription: None,
            setup: None,
            interval: None,
        }
    }

    fn hmac() -> VerificationScheme {
        VerificationScheme::Hmac(HmacSpec {
            algorithm: Digest::Sha256,
            encoding: Encoding::Hex,
            header: "X-Acme-Signature".to_string(),
            prefix: None,
            signed: "{body}".to_string(),
            timestamp: None,
            // GitHub's shape: nothing is timestamped, so there is no spelling to declare.
            timestamp_format: None,
            secret: "acme.signing_secret".to_string(),
            tolerance: None,
        })
    }

    /// **The three states stay three.** Collapsing the unset arm into the explicit one would tell a
    /// consumer that Slack's Socket Mode and an unsigned public endpoint carry the same risk.
    #[test]
    fn the_tri_state_publishes_as_three_distinct_kinds() {
        assert_eq!(
            verification_of(&channel(Transport::Webhook, Some(hmac()))).kind(),
            "hmac"
        );
        assert_eq!(
            verification_of(&channel(Transport::Webhook, Some(VerificationScheme::None))).kind(),
            "none"
        );
        assert_eq!(
            verification_of(&channel(Transport::Socket, None)).kind(),
            "connection"
        );
        assert_eq!(
            verification_of(&channel(Transport::Poll, None)).kind(),
            "connection"
        );
    }

    /// The boolean a consumer filters on is false for exactly one kind, and it is the kind an author
    /// had to write out deliberately.
    #[test]
    fn only_a_deliberately_unverifiable_surface_reads_as_unverified() {
        assert!(
            !verification_of(&channel(Transport::Webhook, Some(VerificationScheme::None)))
                .verified()
        );
        assert!(verification_of(&channel(Transport::Webhook, Some(hmac()))).verified());
        assert!(verification_of(&channel(Transport::Socket, None)).verified());
    }

    /// An unset webhook is a loader error, so it cannot reach a backend — but if one ever did, it
    /// must publish as the *unverified* kind rather than borrow the socket's answer.
    #[test]
    fn an_unset_webhook_never_launders_itself_as_connection_authenticated() {
        let unset = verification_of(&channel(Transport::Webhook, None));
        assert_eq!(unset.kind(), "none");
        assert!(!unset.verified());
    }
}
