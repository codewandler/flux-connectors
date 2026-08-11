//! **An operator-approved HTTPS origin**: one normalized value, parsed once and compared
//! everywhere.
//!
//! A self-managed product cannot name every installation of itself, so a connector declares one
//! complete HTTPS origin and keeps ownership of the API path after it (C-508). That makes the origin
//! the one configuration value which decides *which server* a credential is sent to, and therefore
//! the one whose spelling several independent programs have to agree about: the compiler validates
//! what a provider declares, the runtime pack composes the request and the permission subject from
//! what a tenant supplied, and Exchange persists, compares and approves a proposal before either of
//! them runs.
//!
//! # Why the value is normalized rather than validated
//!
//! A validator answers *may this string be used?* and leaves every consumer holding the string. Two
//! consumers then disagree about whether `https://GitLab.com` and `https://gitlab.com:443` are the
//! same destination — and the disagreement is not academic: one side approves a proposal, the other
//! sees a value that does not equal the approved one and refuses, or worse, treats a re-typed
//! default as a new grant of authority. [`HttpsOrigin`] answers *which destination is this?*
//! instead. Equivalent safe spellings parse to one value, so [`Eq`], [`Ord`] and [`Hash`] compare
//! origins rather than caller spelling, and a different canonical origin stays a real authority
//! change.
//!
//! Canonical rendering is therefore part of identity, not presentation:
//!
//! | input | canonical |
//! |---|---|
//! | `HTTPS://GitLab.com` | `https://gitlab.com` |
//! | `https://gitlab.com:443` | `https://gitlab.com` |
//! | `https://GITLAB.example:08443` | `https://gitlab.example:8443` |
//! | `https://[2001:0db8:0000:0000:0000:0000:0000:0001]` | `https://[2001:db8::1]` |
//!
//! # What it refuses, and what it does not know
//!
//! The value ends after the authority. Userinfo, plain HTTP, a path (`/` included), a query, a
//! fragment, whitespace, a `{placeholder}`, an unbracketed IPv6 literal, a non-ASCII or otherwise
//! malformed host and a zero or out-of-range port are all [`OriginRefusal`]s — the connector owns
//! every byte after the origin, and a value that could replace it would be an unbounded egress
//! grant rather than a configuration field.
//!
//! This type holds **no approval state**. Whether a given canonical origin is *allowed* is a
//! deployment policy question that Exchange owns; all this answers is whether two safe spellings
//! name the same normalized destination.
//!
//! ```
//! use connector_address::{HttpsOrigin, OriginRefusal};
//!
//! // Connection input may arrive in any equivalent safe spelling.
//! let supplied = HttpsOrigin::parse("HTTPS://GitLab.com:443")?;
//! let reviewed = HttpsOrigin::parse("https://gitlab.com")?;
//! assert_eq!(supplied, reviewed);
//! assert_eq!(supplied.as_str(), "https://gitlab.com");
//!
//! // A declaration published into an artifact must already be canonical.
//! assert_eq!(
//!     HttpsOrigin::parse_canonical("HTTPS://GitLab.com:443"),
//!     Err(OriginRefusal::NotCanonical)
//! );
//!
//! // The connector owns the path; a supplied value may not replace it.
//! assert_eq!(
//!     HttpsOrigin::parse("https://gitlab.company.example/api/v4"),
//!     Err(OriginRefusal::Path)
//! );
//! # Ok::<(), OriginRefusal>(())
//! ```

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

/// The only scheme an origin may carry, and the only spelling of it that is canonical.
const SCHEME: &str = "https://";

/// The port HTTPS reaches when none is spelled out, and therefore the one port a canonical origin
/// never renders.
const DEFAULT_PORT: u16 = 443;

/// A normalized HTTPS origin: scheme, host, and a non-default port when there is one.
///
/// Constructed only by [`HttpsOrigin::parse`] or [`HttpsOrigin::parse_canonical`], so a value of
/// this type is a destination that has already been checked and normalized — see the [module
/// documentation](self) for the grammar and the canonical form.
///
/// # It does not render itself by accident
///
/// [`Debug`] is implemented by hand and prints no origin. The customer-supplied authority of a
/// self-managed installation is a deployment detail that has no business appearing in a log line, a
/// `{:?}` of some enclosing structure, or an error chain that happens to include one — and a derived
/// `Debug` puts it in all three without anyone deciding to. Rendering is an explicit call:
/// [`as_str`](Self::as_str) or [`into_string`](Self::into_string).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HttpsOrigin {
    /// The canonical text. Every other field is a function of it, which is what keeps the derived
    /// [`Eq`], [`Ord`] and [`Hash`] a comparison of origins rather than of spellings.
    canonical: String,
    /// The port when it is not the effective default — the same condition under which `canonical`
    /// renders one.
    port: Option<u16>,
}

impl HttpsOrigin {
    /// **Parse and normalize** a supplied origin.
    ///
    /// This is the one entry point for connection input: a tenant, an operator form or a stored
    /// value may spell an origin in any equivalent safe way, and the result is the single normalized
    /// value everything downstream compares, stores and sends.
    ///
    /// # Errors
    ///
    /// [`OriginRefusal`], which names the rejected class and never retains the supplied text.
    pub fn parse(value: &str) -> Result<Self, OriginRefusal> {
        // Whitespace and braces are refused over the whole value rather than per component: a
        // `{placeholder}` is what this value is substituted *into*, so a value spelling one would be
        // filled in twice or reach the vendor verbatim, and neither is a destination anybody chose.
        if value.chars().any(char::is_whitespace) {
            return Err(OriginRefusal::Whitespace);
        }
        if value.contains(['{', '}']) {
            return Err(OriginRefusal::Placeholder);
        }

        let (scheme, authority) = value.split_once("://").ok_or(OriginRefusal::NotHttps)?;
        if !scheme.eq_ignore_ascii_case("https") {
            return Err(OriginRefusal::NotHttps);
        }
        // The first delimiter decides the refusal, so the reason names what the caller actually
        // wrote rather than the first rule that happened to be checked.
        if let Some(index) = authority.find(['/', '?', '#']) {
            return Err(match authority.as_bytes()[index] {
                b'/' => OriginRefusal::Path,
                b'?' => OriginRefusal::Query,
                _ => OriginRefusal::Fragment,
            });
        }
        if authority.contains('@') {
            return Err(OriginRefusal::Userinfo);
        }
        if authority.is_empty() {
            return Err(OriginRefusal::MissingHost);
        }

        let (host, port) = split_authority(authority)?;
        let port = port.map(canonical_port).transpose()?;
        let canonical = match port {
            Some(port) if port != DEFAULT_PORT => format!("{SCHEME}{host}:{port}"),
            _ => format!("{SCHEME}{host}"),
        };
        Ok(Self {
            canonical,
            port: port.filter(|port| *port != DEFAULT_PORT),
        })
    }

    /// **Parse an origin that is required to be canonical already** — a provider-authored `default`,
    /// `example` or choice.
    ///
    /// A declaration is copied verbatim into the connector manifest, the embedded catalogue and the
    /// public catalogue, so a second safe spelling of one origin would ship as a second origin and
    /// the runtime — which normalizes before it compares — would disagree with the artifact about
    /// which value is the reviewed default. Connection input has no such constraint and goes through
    /// [`parse`](Self::parse).
    ///
    /// # Errors
    ///
    /// Everything [`parse`](Self::parse) refuses, plus [`OriginRefusal::NotCanonical`] for a value
    /// that is safe but is not the canonical spelling of itself.
    pub fn parse_canonical(value: &str) -> Result<Self, OriginRefusal> {
        let origin = Self::parse(value)?;
        if origin.canonical != value {
            return Err(OriginRefusal::NotCanonical);
        }
        Ok(origin)
    }

    /// The canonical text: `https://`, the host, and a non-default port.
    ///
    /// Deliberate rendering — see this type's documentation for why it is a call rather than a
    /// `Debug` derive.
    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    /// The canonical text, owned.
    pub fn into_string(self) -> String {
        self.canonical
    }

    /// The host as it appears in the canonical text: a lowercase DNS name, a dotted IPv4 literal, or
    /// a bracketed IPv6 literal.
    ///
    /// Bracketed rather than bare for IPv6, because that is the form that composes back into a URL
    /// and the one this value was parsed from.
    pub fn host(&self) -> &str {
        let authority = self
            .canonical
            .strip_prefix(SCHEME)
            .expect("a canonical origin begins with its scheme");
        match self.port {
            // A rendered port is always the last `:`-delimited component, IPv6 brackets included.
            Some(_) => authority
                .rsplit_once(':')
                .map_or(authority, |(host, _)| host),
            None => authority,
        }
    }

    /// The port when the origin names one that is not the effective default, mirroring exactly what
    /// [`as_str`](Self::as_str) renders.
    ///
    /// `Some(8443)` for `https://gitlab.example:8443`; `None` for both `https://gitlab.example` and
    /// `https://gitlab.example:443`, because those are one destination and one value.
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// The port a request actually reaches: the declared one, or `443`.
    pub fn effective_port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }
}

impl fmt::Debug for HttpsOrigin {
    /// Value-free, on purpose — see the type's documentation.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HttpsOrigin(<origin>)")
    }
}

/// **Why a string is not an HTTPS origin** — the class of the refusal, never the text refused.
///
/// A configured origin is a deployment detail of somebody's private installation, and a refusal is
/// exactly the moment it would otherwise be copied into a log, an error chain, a support ticket and
/// a test failure at once. So no variant carries the supplied value, and none of them can be made
/// to: the classes are the whole of the type, and [`Display`](std::fmt::Display) and [`Debug`] both
/// render only the class.
///
/// **Closed on purpose.** An unknown spelling has to be a loud refusal rather than a variant a
/// consumer silently maps to "something else went wrong" — matching this exhaustively is a
/// supported thing to do, and a new class arriving is a change consumers should see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, thiserror::Error)]
pub enum OriginRefusal {
    /// No `https://` scheme. Plain HTTP would carry the connection's credential in cleartext.
    #[error(
        "an origin must begin with `https://`; plain HTTP would carry a credential over the \
         network in the clear"
    )]
    NotHttps,
    /// A `user@host` authority. Userinfo moves the host the request — and the credential — reaches.
    #[error(
        "an origin carries no userinfo; a `user:password@host` value moves the authority a request \
         and its credential reach"
    )]
    Userinfo,
    /// A path, `/` included. The connector owns every byte after the origin.
    #[error("an origin ends after its authority and carries no path; the connector owns the path")]
    Path,
    /// A query string. It belongs to the operation, not to the destination.
    #[error("an origin ends after its authority and carries no query")]
    Query,
    /// A fragment. It never reaches a server at all.
    #[error("an origin ends after its authority and carries no fragment")]
    Fragment,
    /// Whitespace anywhere in the value.
    #[error("an origin contains no whitespace")]
    Whitespace,
    /// A `{` or `}`. An origin is a resolved value, not a template.
    #[error(
        "an origin is a resolved value rather than a template, so it may not contain `{{` or `}}`"
    )]
    Placeholder,
    /// An empty authority: `https://`, or `https://:8443`.
    #[error("an origin must name a host")]
    MissingHost,
    /// A host that is neither an ASCII DNS name nor an IP literal — an empty label, a leading or
    /// trailing `-`, a non-ASCII character, or any other byte outside the DNS alphabet.
    #[error("the origin host is not an ASCII DNS name or IP address")]
    UnknownHost,
    /// An IPv6 literal written without brackets, which has no port position: `https://2001:db8::1`.
    #[error("an IPv6 origin host must be enclosed in `[` and `]`, which is where its port goes")]
    UnbracketedIpv6,
    /// A bracketed host that is not an IPv6 address, or a `[` that is never closed.
    #[error("the bracketed origin host is not a closed IPv6 address")]
    InvalidIpv6,
    /// A port that is not a decimal number from 1 to 65535, or trailing text where a port belongs.
    #[error("an origin port must be a decimal number from 1 to 65535")]
    InvalidPort,
    /// A safe origin that is not the canonical spelling of itself, refused where a canonical
    /// declaration is required — see [`HttpsOrigin::parse_canonical`].
    #[error(
        "an origin declaration must already be canonical: a lowercase scheme and DNS host, the \
         standard spelling of an IP literal, and no default `:443` port"
    )]
    NotCanonical,
}

/// Split an authority into its canonical host and its port text, refusing an IPv6 literal that is
/// not bracketed.
///
/// Brackets are what give an IPv6 authority a port position at all, which is why an unbracketed one
/// is a refusal with its own name rather than a malformed host: `https://2001:db8::1` reads as host
/// `2001:db8:` and port `:1` to anything that splits on the last colon.
fn split_authority(authority: &str) -> Result<(String, Option<&str>), OriginRefusal> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (literal, tail) = rest.split_once(']').ok_or(OriginRefusal::InvalidIpv6)?;
        let address: Ipv6Addr = literal.parse().map_err(|_| OriginRefusal::InvalidIpv6)?;
        let port = match tail {
            "" => None,
            tail => Some(tail.strip_prefix(':').ok_or(OriginRefusal::InvalidPort)?),
        };
        return Ok((format!("[{address}]"), port));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, _)) if host.contains(':') => return Err(OriginRefusal::UnbracketedIpv6),
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    Ok((canonical_host(host)?, port))
}

/// One DNS name or IPv4 literal, in its canonical spelling.
///
/// An IPv4 literal is rendered by the standard library, which is what makes `010.1.1.1` a refusal
/// rather than a second spelling of `10.1.1.1`. A DNS name is lowercased, because DNS is
/// case-insensitive and two cases of one name are one destination. A host that parses as neither —
/// `1.2.3.4.5`, say — stays a DNS name if its labels are well formed, exactly as it did before this
/// type existed: an internal or VPN-only name is a real deployment, and narrowing the grammar to
/// public DNS would refuse one.
fn canonical_host(host: &str) -> Result<String, OriginRefusal> {
    if host.is_empty() {
        return Err(OriginRefusal::MissingHost);
    }
    if let Ok(address) = host.parse::<Ipv4Addr>() {
        return Ok(address.to_string());
    }
    for label in host.split('.') {
        if label.is_empty()
            || label.starts_with('-')
            || label.ends_with('-')
            || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(OriginRefusal::UnknownHost);
        }
    }
    Ok(host.to_ascii_lowercase())
}

/// The port a canonical origin would render for this text.
///
/// Digits only, so `+443` and `443 ` — both of which `u16::from_str` and a trimming parser
/// respectively would accept — are refused rather than normalized into a port nobody typed. Leading
/// zeros *are* accepted and normalized, because `:0443` is a spelling of one destination rather than
/// a different one.
fn canonical_port(port: &str) -> Result<u16, OriginRefusal> {
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(OriginRefusal::InvalidPort);
    }
    match port.parse::<u16>() {
        Ok(0) | Err(_) => Err(OriginRefusal::InvalidPort),
        Ok(port) => Ok(port),
    }
}
