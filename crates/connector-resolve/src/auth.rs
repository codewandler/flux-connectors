//! The two axes of authentication: **acquisition** (what the stored value becomes) and
//! **placement** (where it goes on the request).
//!
//! Moved from `connector-pack` unchanged (C-538). It moved because the *plan* carries the placed
//! credential and the redaction set, and a plan whose auth was applied somewhere else would be a
//! second request-composition path — the thing this family already rejected.
//!
//! # What is registered with a redactor, and what is not
//!
//! A header prefix leaves the credential verbatim inside the header, so a redactor holding the bare
//! value already scrubs every surface the prefixed value reaches; registering `SSWS ` would scrub a
//! public word out of unrelated prose and leave the bare token — the form a vendor's 401 echoes
//! back — unregistered. A **query** placement percent-encodes, and `+`, `/` and `=` do not survive
//! that unchanged, which is exactly the alphabet a base64 credential is made of. [`placed_form`] is
//! the one place that answers "does this placement transform the value", and its match over
//! [`Placement`] is exhaustive so a variant added later has to say.

use catalog::{Acquisition, Credential, Placement};

use crate::request::Request;
use crate::Error;

/// The value a credential contributes to a request, and where it goes.
///
/// Split from the raw secret deliberately: [`acquire`] is a pure function of the stored value, so
/// the caller can register **both** strings with the redactor before any request exists, and
/// [`place`] is then a total function that cannot fail on a value it has not seen.
#[derive(Clone, PartialEq, Eq)]
pub struct Assembled {
    /// The credential's flat-namespace name, for a refusal that says which one.
    credential: &'static str,
    /// The acquired value, before any placement prefix.
    value: String,
    /// Where it goes.
    place: Placement,
}

impl Assembled {
    /// Declare that `value` is `credential`'s assembled form and goes at `place`.
    pub fn new(credential: &'static str, value: String, place: Placement) -> Self {
        Self {
            credential,
            value,
            place,
        }
    }

    /// The credential's flat-namespace name.
    pub fn credential(&self) -> &'static str {
        self.credential
    }

    /// **The assembled value.** Named `expose_*` for the same reason
    /// [`SensitiveText::expose_secret`](crate::SensitiveText::expose_secret) is: every call site is
    /// a place a reviewer should stop at.
    pub fn expose_value(&self) -> &str {
        &self.value
    }

    /// Where it goes.
    pub fn placement(&self) -> Placement {
        self.place
    }
}

/// **Hand-written, and the `value` does not print.** A derived `Debug` here would render the
/// assembled plaintext — `Bearer `'s token, or the base64 of a basic pair, which is as good as the
/// secret to anyone holding it. The credential's *name* and its *placement* stay, because they are
/// what a `Debug` of this type is read for.
impl std::fmt::Debug for Assembled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Assembled")
            .field("credential", &self.credential)
            .field("value", &format_args!("<redacted>"))
            .field("place", &self.place)
            .finish()
    }
}

/// Run the acquisition axis: turn the stored value into the value that travels.
///
/// `user` is the resolved user half for [`Acquisition::BasicJoin`], already carrying its literal
/// suffix. It is passed in rather than read here so that this stays a pure function.
pub fn acquire(credential: &'static Credential, secret: &str, user: Option<&str>) -> String {
    match credential.acquire {
        Acquisition::Static => secret.to_string(),
        // Identical to `Static`, and deliberately spelled out rather than joined to it (C-136): a
        // minted credential is a stored value like any other by the time anything places it.
        Acquisition::Minted { .. } => secret.to_string(),
        // Identical for the same reason (C-525): the stored value is an access token the *host*
        // obtained by running the grant, and this crate opens no socket.
        Acquisition::OAuth2(_) => secret.to_string(),
        // A `None` user cannot happen — the caller resolves it whenever the acquisition is a join.
        Acquisition::BasicJoin { .. } => {
            base64(format!("{}:{}", user.unwrap_or_default(), secret).as_bytes())
        }
    }
}

/// Run the placement axis: put `assembled` onto `request`.
///
/// # Errors
///
/// [`Error::InboundCredential`] for a signing secret, which verifies bytes that arrived and must
/// never leave, and [`Error::CredentialCollision`] when the operation's own request template already
/// sets the header the credential would occupy.
pub fn place(operation: &str, assembled: &Assembled, request: &mut Request) -> Result<(), Error> {
    match assembled.place {
        Placement::Header { name, prefix } => {
            if let Some(existing) = request
                .headers
                .keys()
                .find(|header| header.eq_ignore_ascii_case(name))
            {
                return Err(Error::CredentialCollision {
                    operation: operation.to_owned(),
                    credential: assembled.credential.to_owned(),
                    header: existing.clone(),
                });
            }
            request
                .headers
                .insert(name.to_string(), format!("{prefix}{}", assembled.value));
            Ok(())
        }
        Placement::Query { name } => {
            let separator = if request.url.contains('?') { '&' } else { '?' };
            request.url.push(separator);
            request.url.push_str(name);
            request.url.push('=');
            // The same encoder [`placed_form`] hands to the redactor, so the string on the wire is
            // the string that was registered rather than a second derivation of it.
            request.url.push_str(&query_encode(&assembled.value));
            Ok(())
        }
        Placement::Inbound => Err(Error::InboundCredential {
            operation: operation.to_owned(),
            credential: assembled.credential.to_owned(),
        }),
    }
}

/// **The form of `value` a placement puts on the wire, when that is not `value` itself.**
///
/// `None` when the placement only *surrounds* the value; `Some` when it **transforms** it.
pub fn placed_form(place: Placement, value: &str) -> Option<String> {
    match place {
        Placement::Query { .. } => Some(query_encode(value)),
        Placement::Header { .. } | Placement::Inbound => None,
    }
}

/// Standard base64 with padding (RFC 4648 §4).
///
/// Hand-rolled rather than taken as a dependency: this crate's claim is that it links no transport
/// and does nothing a reader has to take on trust, and 20 lines of table lookup is cheaper to audit
/// than a supply-chain edge added for one call site.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let bits = (u32::from(chunk[0]) << 16)
            | (u32::from(chunk.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(ALPHABET[((bits >> (18 - index * 6)) & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Percent-encode everything outside RFC 3986's *unreserved* set.
///
/// A credential in a query string is the one value here that a caller never sees and cannot fix, so
/// it is encoded rather than trusted to be URL-safe. It is also the single encoding boundary for a
/// structured query, so nothing else on the URL encodes.
pub fn query_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET";

    fn request() -> Request {
        Request {
            method: "GET".to_string(),
            url: "https://vendor.example/api/v2/things".to_string(),
            headers: BTreeMap::new(),
            body: None,
        }
    }

    fn credential(acquire: Acquisition, place: Placement) -> &'static Credential {
        Box::leak(Box::new(Credential {
            name: "acme.token",
            leaf: "token",
            acquire,
            place,
            // This module is the *placement* half, and placement is subject-independent: an app
            // token and a user token go onto a request identically.
            subject: catalog::Subject::Unstated,
            hazard: None,
        }))
    }

    #[test]
    fn base64_matches_rfc_4648s_own_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_prefixed_header_carries_the_bare_value_inside_it() {
        let assembled = Assembled::new(
            "acme.token",
            SENTINEL.to_string(),
            Placement::Header {
                name: "Authorization",
                prefix: "Bearer ",
            },
        );
        let mut request = request();
        place("acme-thing-list", &assembled, &mut request).expect("it places");
        assert_eq!(
            request.headers.get("Authorization").map(String::as_str),
            Some(format!("Bearer {SENTINEL}").as_str())
        );
        assert_eq!(placed_form(assembled.placement(), SENTINEL), None);
    }

    #[test]
    fn a_query_placement_registers_the_encoded_form_it_sends() {
        let value = "a+b/c=";
        let assembled = Assembled::new(
            "acme.token",
            value.to_string(),
            Placement::Query { name: "api_key" },
        );
        let mut request = request();
        place("acme-thing-list", &assembled, &mut request).expect("it places");
        let encoded = placed_form(assembled.placement(), value).expect("query transforms");
        assert!(
            request.url.ends_with(&format!("?api_key={encoded}")),
            "{}",
            request.url
        );
    }

    #[test]
    fn a_header_the_template_already_sets_is_refused_rather_than_overwritten() {
        let assembled = Assembled::new(
            "acme.token",
            SENTINEL.to_string(),
            Placement::Header {
                name: "Authorization",
                prefix: "",
            },
        );
        let mut request = request();
        request
            .headers
            .insert("authorization".to_string(), "declared".to_string());
        assert!(matches!(
            place("acme-thing-list", &assembled, &mut request),
            Err(Error::CredentialCollision { .. })
        ));
    }

    #[test]
    fn an_inbound_signing_secret_never_leaves() {
        let assembled = Assembled::new("acme.sig", SENTINEL.to_string(), Placement::Inbound);
        assert!(matches!(
            place("acme-thing-list", &assembled, &mut request()),
            Err(Error::InboundCredential { .. })
        ));
    }

    #[test]
    fn a_basic_join_composes_the_pair_the_vendor_expects() {
        let joined = acquire(
            credential(
                Acquisition::BasicJoin {
                    user_env: &["ACME_USER"],
                    user_suffix: "",
                },
                Placement::Header {
                    name: "Authorization",
                    prefix: "Basic ",
                },
            ),
            "secret",
            Some("ops@acme.test"),
        );
        assert_eq!(joined, base64(b"ops@acme.test:secret"));
    }

    #[test]
    fn the_assembled_debug_prints_no_value() {
        let assembled = Assembled::new(
            "acme.token",
            SENTINEL.to_string(),
            Placement::Query { name: "api_key" },
        );
        let printed = format!("{assembled:?}");
        assert!(!printed.contains(SENTINEL), "{printed}");
        assert!(printed.contains("acme.token"), "{printed}");
    }
}
