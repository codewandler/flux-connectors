//! **One corpus of HTTPS origins**, read by the value type that owns the grammar, by the compiler
//! that validates a declaration, and by the runtime pack that composes a request from a supplied
//! value.
//!
//! Every case states an input and what it is worth: the canonical origin it normalizes to, or the
//! [`OriginRefusal`] class it is refused as. That is deliberately more than the accepted/refused
//! table this replaced (C-508's `origin_grammar.rs`) — a table of *which values are legal* cannot
//! say that two legal values are the same destination, which is the property the three consumers
//! actually have to agree about (C-523).
//!
//! The three read it for three different claims, and the difference is the contract:
//!
//! | consumer | what the corpus means there |
//! |---|---|
//! | `connector-address` | `parse` returns exactly this canonical text or exactly this refusal |
//! | `connector-spec` | a **declaration** is accepted only where the input already *is* its canonical text |
//! | `connector-pack` | a supplied value is normalized before it becomes a destination, a permission subject or an approval comparison |
//!
//! This module contains data only. It lives beside the crate that owns the type so that a consumer
//! cannot quietly grow a second grammar to match its own copy of the table.

// Each consumer reads the subset of this module its own claim needs — the pack never asks whether an
// input is *declarable*, and the loader never composes a request from the canonical text — so an
// unused item here is the fixture being shared rather than something nobody uses. Trimming it to
// whichever consumer compiles last is how one corpus becomes three.
#![allow(dead_code)]

use connector_address::OriginRefusal;

/// One input and what the shared contract says it is worth.
#[derive(Debug, Clone, Copy)]
pub struct OriginCase {
    /// The value as a caller supplies it.
    pub input: &'static str,
    /// What parsing it produces.
    pub outcome: Outcome,
}

/// The two things an input can be worth.
#[derive(Debug, Clone, Copy)]
pub enum Outcome {
    /// A safe origin, and the canonical text it normalizes to. Equal to `input` exactly when the
    /// input is already canonical, which is the form a declaration must take.
    Canonical(&'static str),
    /// Not an origin, and the class it is refused as.
    Refused(OriginRefusal),
}

impl OriginCase {
    /// The canonical text, for an accepted case.
    pub fn canonical(&self) -> Option<&'static str> {
        match self.outcome {
            Outcome::Canonical(canonical) => Some(canonical),
            Outcome::Refused(_) => None,
        }
    }

    /// Whether the input is already the canonical spelling of itself — the condition a
    /// provider-authored `default`, `example` or choice must satisfy.
    pub fn is_canonical_declaration(&self) -> bool {
        self.canonical() == Some(self.input)
    }
}

/// The corpus.
///
/// Ordered by what each case is for: the canonical forms first, then the equivalent spellings that
/// must collapse onto them, then the refusals grouped by class.
pub const ORIGIN_CASES: &[OriginCase] = &[
    // ---- Canonical forms. A declaration may carry any of these verbatim. ----
    OriginCase {
        input: "https://gitlab.com",
        outcome: Outcome::Canonical("https://gitlab.com"),
    },
    OriginCase {
        input: "https://gitlab.company.example:8443",
        outcome: Outcome::Canonical("https://gitlab.company.example:8443"),
    },
    // A single label is a real deployment: an internal or VPN-only name has no public DNS suffix,
    // and a grammar narrowed to public names would refuse the installation it exists for.
    OriginCase {
        input: "https://localhost",
        outcome: Outcome::Canonical("https://localhost"),
    },
    OriginCase {
        input: "https://gitlab-internal",
        outcome: Outcome::Canonical("https://gitlab-internal"),
    },
    OriginCase {
        input: "https://gitlab.internal",
        outcome: Outcome::Canonical("https://gitlab.internal"),
    },
    // A self-managed instance at a multi-label corporate name, which is the ordinary shape of a
    // self-hosted forge and the case a public-DNS-only grammar would wrongly refuse. The host is
    // `.example` (RFC 2606) deliberately: a corpus is a committed, published file, and a real
    // deployment's hostname is infrastructure detail that does not belong in one.
    OriginCase {
        input: "https://gitlab.stack.example",
        outcome: Outcome::Canonical("https://gitlab.stack.example"),
    },
    OriginCase {
        input: "https://10.42.0.7:8443",
        outcome: Outcome::Canonical("https://10.42.0.7:8443"),
    },
    OriginCase {
        input: "https://127.0.0.1",
        outcome: Outcome::Canonical("https://127.0.0.1"),
    },
    OriginCase {
        input: "https://[2001:db8::1]:8443",
        outcome: Outcome::Canonical("https://[2001:db8::1]:8443"),
    },
    OriginCase {
        input: "https://[2001:db8::1]",
        outcome: Outcome::Canonical("https://[2001:db8::1]"),
    },
    // ---- Equivalent safe spellings. Accepted from a connection, refused in a declaration. ----
    OriginCase {
        input: "HTTPS://gitlab.com",
        outcome: Outcome::Canonical("https://gitlab.com"),
    },
    OriginCase {
        input: "https://GitLab.COM",
        outcome: Outcome::Canonical("https://gitlab.com"),
    },
    // The effective default port is omitted, so spelling it out names the same destination.
    OriginCase {
        input: "https://gitlab.com:443",
        outcome: Outcome::Canonical("https://gitlab.com"),
    },
    OriginCase {
        input: "https://gitlab.com:0443",
        outcome: Outcome::Canonical("https://gitlab.com"),
    },
    OriginCase {
        input: "https://gitlab.company.example:08443",
        outcome: Outcome::Canonical("https://gitlab.company.example:8443"),
    },
    OriginCase {
        input: "HTTPS://GitLab.Company.Example:443",
        outcome: Outcome::Canonical("https://gitlab.company.example"),
    },
    // IPv6 compression and hexadecimal case are spelling, not identity.
    OriginCase {
        input: "https://[2001:0db8:0000:0000:0000:0000:0000:0001]",
        outcome: Outcome::Canonical("https://[2001:db8::1]"),
    },
    OriginCase {
        input: "https://[2001:DB8::1]:8443",
        outcome: Outcome::Canonical("https://[2001:db8::1]:8443"),
    },
    OriginCase {
        input: "https://[::1]:443",
        outcome: Outcome::Canonical("https://[::1]"),
    },
    // ---- Refusals. ----
    OriginCase {
        input: "http://gitlab.company.example",
        outcome: Outcome::Refused(OriginRefusal::NotHttps),
    },
    OriginCase {
        input: "HTTP://gitlab.company.example",
        outcome: Outcome::Refused(OriginRefusal::NotHttps),
    },
    OriginCase {
        input: "gitlab.company.example",
        outcome: Outcome::Refused(OriginRefusal::NotHttps),
    },
    OriginCase {
        input: "https:/gitlab.company.example",
        outcome: Outcome::Refused(OriginRefusal::NotHttps),
    },
    OriginCase {
        input: "https://",
        outcome: Outcome::Refused(OriginRefusal::MissingHost),
    },
    OriginCase {
        input: "https://:8443",
        outcome: Outcome::Refused(OriginRefusal::MissingHost),
    },
    OriginCase {
        input: "https://user@gitlab.company.example",
        outcome: Outcome::Refused(OriginRefusal::Userinfo),
    },
    OriginCase {
        input: "https://gitlab.company.example@egress.example",
        outcome: Outcome::Refused(OriginRefusal::Userinfo),
    },
    // A trailing slash is a path: the connector owns everything after the authority, and an empty
    // path is still the caller deciding where the connector's own path starts.
    OriginCase {
        input: "https://gitlab.company.example/",
        outcome: Outcome::Refused(OriginRefusal::Path),
    },
    OriginCase {
        input: "https://gitlab.company.example/api/v4",
        outcome: Outcome::Refused(OriginRefusal::Path),
    },
    OriginCase {
        input: "https://gitlab.company.example?path=/api/v4",
        outcome: Outcome::Refused(OriginRefusal::Query),
    },
    OriginCase {
        input: "https://gitlab.company.example#api-v4",
        outcome: Outcome::Refused(OriginRefusal::Fragment),
    },
    OriginCase {
        input: "https://gitlab company.example",
        outcome: Outcome::Refused(OriginRefusal::Whitespace),
    },
    OriginCase {
        input: " https://gitlab.company.example",
        outcome: Outcome::Refused(OriginRefusal::Whitespace),
    },
    OriginCase {
        input: "https://{origin}.example",
        outcome: Outcome::Refused(OriginRefusal::Placeholder),
    },
    OriginCase {
        input: "https://-gitlab.example",
        outcome: Outcome::Refused(OriginRefusal::UnknownHost),
    },
    OriginCase {
        input: "https://gitlab-.example",
        outcome: Outcome::Refused(OriginRefusal::UnknownHost),
    },
    OriginCase {
        input: "https://gitlab..example",
        outcome: Outcome::Refused(OriginRefusal::UnknownHost),
    },
    OriginCase {
        input: "https://gitlab.example.",
        outcome: Outcome::Refused(OriginRefusal::UnknownHost),
    },
    OriginCase {
        input: "https://gitlab_internal.example",
        outcome: Outcome::Refused(OriginRefusal::UnknownHost),
    },
    // A non-ASCII host is refused rather than transcoded: this crate does not implement IDNA, and
    // guessing at one is how a homograph reaches a destination nobody reviewed.
    OriginCase {
        input: "https://gitlab.ëxample",
        outcome: Outcome::Refused(OriginRefusal::UnknownHost),
    },
    OriginCase {
        input: "https://2001:db8::1",
        outcome: Outcome::Refused(OriginRefusal::UnbracketedIpv6),
    },
    OriginCase {
        input: "https://[2001:db8::1",
        outcome: Outcome::Refused(OriginRefusal::InvalidIpv6),
    },
    OriginCase {
        input: "https://[not-ipv6]",
        outcome: Outcome::Refused(OriginRefusal::InvalidIpv6),
    },
    OriginCase {
        input: "https://gitlab.example:",
        outcome: Outcome::Refused(OriginRefusal::InvalidPort),
    },
    OriginCase {
        input: "https://gitlab.example:0",
        outcome: Outcome::Refused(OriginRefusal::InvalidPort),
    },
    OriginCase {
        input: "https://gitlab.example:65536",
        outcome: Outcome::Refused(OriginRefusal::InvalidPort),
    },
    OriginCase {
        input: "https://gitlab.example:+443",
        outcome: Outcome::Refused(OriginRefusal::InvalidPort),
    },
    OriginCase {
        input: "https://gitlab.example:8443x",
        outcome: Outcome::Refused(OriginRefusal::InvalidPort),
    },
    OriginCase {
        input: "https://[2001:db8::1]8443",
        outcome: Outcome::Refused(OriginRefusal::InvalidPort),
    },
];
