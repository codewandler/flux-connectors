//! Global addresses for a provider, one of its services, and one operation.
//!
//! ```text
//! pid   com.amazonaws                             the provider
//! gid   com.amazonaws/s3:2006-03-01               one service of it, versioned
//! oip   com.amazonaws/s3:2006-03-01#object-get    one operation
//! ```
//!
//! ```text
//! pid  := <authority>
//! gid  := <authority> [ "/" <service> ] ":" <api-version>
//! oip  := <gid> "#" <operation>
//! ```
//!
//! Each separator carries exactly one meaning — `/` hierarchy, `:` version, `#` operation — mirroring
//! URI path-then-fragment syntax. The scheme is C-37's
//! ([`docs/designs/global-addressing.md`](../../../docs/designs/global-addressing.md)); what C-49
//! fixes is the **meaning of the middle level**: it is a declared
//! [`Service`](crate::Service), not an anonymous path segment, and it owns the version that renders
//! after the colon (`docs/designs/provider-services.md`).
//!
//! # The elision is the load-bearing rule
//!
//! [`DEFAULT_SERVICE`] is **never rendered**. `com.freshdesk.api:v2`, not
//! `com.freshdesk.api/default:v2`. `default` is an internal name for "this provider has one API
//! surface"; an address is a promise, and that one would have to be broken the day the provider grows
//! a second service. `parse(render(x)) == x` holds through the elision because the grammar has exactly
//! one optional middle segment: no segment means the default service, one segment names it.
//!
//! **This is the constraint C-37 must respect.** When C-37 adds its remaining path segments below the
//! service, `com.freshdesk.api/tickets:v2` becomes ambiguous — `tickets` could be the service, or a
//! tail segment under an elided `default`. The design records the two admissible resolutions; neither
//! is decided here, and a gid with more than one middle segment is refused rather than guessed at.
//!
//! # Rendered, never authored
//!
//! An address is a *function* of structured fields ([`Connector::gid_of`](crate::Connector::gid_of)),
//! so a typo in a segment cannot masquerade as a valid address, and grouping or filtering by service
//! needs no parser at the call site. [`Gid::parse`] exists for the other direction — a user or an
//! external reference naming a slice — and it validates every component.

use std::fmt;

use crate::ir::DEFAULT_SERVICE;

/// A provider's address: its reverse-DNS authority, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid {
    /// The reverse-DNS authority, e.g. `com.amazonaws`.
    pub authority: String,
}

impl Pid {
    /// A pid for `authority`, unvalidated — the constructor a renderer uses on fields the loader has
    /// already checked. [`Pid::parse`] is the validating direction.
    pub fn new(authority: &str) -> Self {
        Self {
            authority: authority.to_owned(),
        }
    }

    /// Parse `com.amazonaws`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidAddress`](crate::Error::InvalidAddress) when the authority is not a
    /// reverse-DNS label sequence.
    pub fn parse(text: &str) -> crate::Result<Self> {
        validate_authority(text)?;
        Ok(Self::new(text))
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.authority)
    }
}

/// One service of one provider, at one version: the middle level, and the unit you install.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gid {
    /// The provider's reverse-DNS authority, e.g. `com.amazonaws`.
    pub authority: String,
    /// The service name, e.g. `s3`. [`DEFAULT_SERVICE`] here renders as nothing at all.
    pub service: String,
    /// The **vendor's** API version for this service, e.g. `2006-03-01` or `v2`. Never ours: our own
    /// connector version lives in `connectors.lock`, so an address is stable across regenerations.
    pub api_version: String,
}

impl Gid {
    /// A gid from already-validated fields.
    pub fn new(authority: &str, service: &str, api_version: &str) -> Self {
        Self {
            authority: authority.to_owned(),
            service: service.to_owned(),
            api_version: api_version.to_owned(),
        }
    }

    /// The provider this service belongs to.
    pub fn pid(&self) -> Pid {
        Pid::new(&self.authority)
    }

    /// Whether this gid addresses the reserved default service, and therefore renders with no middle
    /// segment.
    pub fn is_default_service(&self) -> bool {
        self.service == DEFAULT_SERVICE
    }

    /// Parse `com.amazonaws/s3:2006-03-01`, or `com.freshdesk.api:v2` for the default service.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidAddress`](crate::Error::InvalidAddress) when a component is malformed, when the
    /// version is missing, when the elided default is written out explicitly, or when more than one
    /// middle segment is present — see the module docs on why that last case is refused rather than
    /// interpreted.
    pub fn parse(text: &str) -> crate::Result<Self> {
        if text.contains('#') {
            return Err(invalid(
                text,
                "a `#` introduces an operation, which makes this an oip rather than a gid — parse it \
                 with `Oip::parse`",
            ));
        }
        let Some((path, api_version)) = text.rsplit_once(':') else {
            return Err(invalid(
                text,
                "a gid names the vendor's API version after a `:`, as in \
                 `com.amazonaws/s3:2006-03-01`",
            ));
        };
        validate_version(text, api_version)?;

        let mut segments = path.split('/');
        let authority = segments.next().unwrap_or_default();
        validate_authority(authority)?;

        let service = match (segments.next(), segments.next()) {
            (None, _) => DEFAULT_SERVICE.to_owned(),
            (Some(service), None) => {
                validate_service(text, service)?;
                if service == DEFAULT_SERVICE {
                    return Err(invalid(
                        text,
                        "the reserved `default` service is elided from an address, so it must not be \
                         written out — drop the segment",
                    ));
                }
                service.to_owned()
            }
            (Some(_), Some(_)) => return Err(invalid(
                text,
                "a gid has exactly one service segment. Deeper resource paths are C-37's and are \
                     not part of this grammar yet — see `docs/designs/provider-services.md`",
            )),
        };

        Ok(Self {
            authority: authority.to_owned(),
            service,
            api_version: api_version.to_owned(),
        })
    }
}

impl fmt::Display for Gid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.authority)?;
        if !self.is_default_service() {
            write!(f, "/{}", self.service)?;
        }
        write!(f, ":{}", self.api_version)
    }
}

/// One operation, addressed globally: a [`Gid`] plus the operation's local symbol.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oip {
    /// The service the operation belongs to, versioned.
    pub gid: Gid,
    /// The operation's id — the declarable Flux symbol, unchanged by this scheme.
    pub operation: String,
}

impl Oip {
    /// An oip from already-validated parts.
    pub fn new(gid: Gid, operation: &str) -> Self {
        Self {
            gid,
            operation: operation.to_owned(),
        }
    }

    /// Parse `com.amazonaws/s3:2006-03-01#object-get`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidAddress`](crate::Error::InvalidAddress) when the `#` is missing, when the
    /// operation part is empty, or when the gid before it is malformed.
    pub fn parse(text: &str) -> crate::Result<Self> {
        let Some((gid, operation)) = text.split_once('#') else {
            return Err(invalid(
                text,
                "an oip names its operation after a `#`, as in \
                 `com.amazonaws/s3:2006-03-01#object-get`",
            ));
        };
        if operation.is_empty() {
            return Err(invalid(text, "the operation after the `#` is empty"));
        }
        if operation.contains('#') {
            return Err(invalid(text, "an oip carries exactly one `#`"));
        }
        Ok(Self {
            gid: Gid::parse(gid)?,
            operation: operation.to_owned(),
        })
    }
}

impl fmt::Display for Oip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.gid, self.operation)
    }
}

/// An authority is a reverse-DNS label sequence: two or more lowercase labels joined by dots.
///
/// `com.amazonaws`, not `amazonaws` and not `Com.Amazonaws`. A single label would be a name in no
/// namespace, which is the collision space reverse-DNS exists to make obvious.
fn validate_authority(authority: &str) -> crate::Result<()> {
    if authority.is_empty() {
        return Err(invalid(authority, "an address begins with an authority"));
    }
    let labels: Vec<&str> = authority.split('.').collect();
    if labels.len() < 2 {
        return Err(invalid(
            authority,
            "an authority is a reverse-DNS label sequence of at least two labels, as in \
             `com.amazonaws`",
        ));
    }
    for label in labels {
        if label.is_empty() {
            return Err(invalid(authority, "an authority has an empty label"));
        }
        if !label
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(invalid(
                authority,
                "authority labels are lowercase ASCII letters, digits and `-`",
            ));
        }
    }
    Ok(())
}

/// A service segment is lowercase kebab: `s3`, `bedrock-runtime`.
fn validate_service(address: &str, service: &str) -> crate::Result<()> {
    if service.is_empty() {
        return Err(invalid(address, "the service segment is empty"));
    }
    if !service
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(invalid(
            address,
            "a service name is lowercase ASCII letters, digits and `-`",
        ));
    }
    Ok(())
}

/// The version is the vendor's own spelling — `v2`, `2006-03-01`, `2023-09-30` — so it is checked for
/// being present and free of the scheme's separators, not for matching a shape we invented.
fn validate_version(address: &str, api_version: &str) -> crate::Result<()> {
    if api_version.is_empty() {
        return Err(invalid(address, "the API version after the `:` is empty"));
    }
    if api_version.contains('/') {
        return Err(invalid(
            address,
            "the API version must not contain `/`; the `:` comes after the last path segment",
        ));
    }
    Ok(())
}

fn invalid(address: &str, reason: &str) -> crate::Error {
    crate::Error::InvalidAddress {
        address: address.to_owned(),
        reason: reason.to_owned(),
    }
}
