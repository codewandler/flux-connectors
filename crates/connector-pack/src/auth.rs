//! **Assembling a vendor's auth in Rust** — the `Bearer` prefix, the basic-auth base64, the
//! query-parameter placement.
//!
//! # Why here and not in flux
//!
//! flux's `{"$secret": "ENV"}` marker is *whole-value, headers-only, no prefix, no encode*, so
//! `Authorization: Bearer <t>` and `Basic base64(u:p)` are both inexpressible with it. That is the
//! `$auth` seam ([`auth-seam.md`](../../../docs/designs/auth-seam.md)), and it is why *"no provider
//! can make a live call"* has been an intentional gap since C-10.
//!
//! A Tool does not need the seam: it builds its own header value before `http.request` ever sees the
//! request. So the marker never has to grow those capabilities, and milestone 1 stops waiting on a
//! flux release. What the seam still buys is the *composite* path, which is why it is narrowed
//! rather than deleted.
//!
//! # The three axes, and where each one lives
//!
//! `docs/designs/unified-auth.md` cuts a credential into **source × acquisition × placement**, and
//! this module is the second and third:
//!
//! | axis | who owns it |
//! |---|---|
//! | source — where raw material comes from | [`crate::Credentials`], through `connector-secrets` |
//! | acquisition — how it becomes usable | [`acquire`], over [`catalog::Acquisition`] |
//! | placement — where it goes on the wire | [`place`], over [`catalog::Placement`] |
//!
//! The design's central claim is that `prefix` is data: `Bearer `, `Basic `, `Token ` and the empty
//! prefix are one code path here, not four. The value of that is visible below as the absence of a
//! `Bearer` arm — [`place`] has no idea what a bearer token is.
//!
//! # The user half of Basic is config, not a secret
//!
//! `base64(<user><suffix>:<secret>)` needs a username, and for every provider that can express it
//! the username is an email address or an account name — config, resolved from the declared
//! environment variables, exactly as flux resolves its own `AuthMethod::user_env`. Only the password
//! half comes from the secret store.
//!
//! **The composed blob is itself a credential.** `base64("me@corp.com/token:<secret>")` is as good
//! as the secret to anyone holding it, so [`crate::Credentials`] registers the assembled value with
//! the redactor as well as the raw one; registering only the raw secret would leave the thing that
//! actually travels unredacted.
//!
//! # What is registered with the redactor, and why a prefix does not change it (C-184)
//!
//! The rule is one line: **the redactor holds every value that is not recoverable from a value it
//! already holds.** Registration happens on the *acquisition* axis, never on the placement axis, and
//! the two axes divide cleanly on that test:
//!
//! - [`acquire`] can **transform** the secret. `base64(user:secret)` does not contain the secret as
//!   a substring, so scrubbing the secret would leave the travelling blob intact — hence the second
//!   registration in [`crate::Credentials`].
//! - [`place`] only **surrounds** it. `SSWS <token>` contains `<token>` verbatim, so a redactor
//!   holding the bare token already scrubs every surface the prefixed value reaches; what survives
//!   is `SSWS <redacted>`, which is the scheme word the vendor prints in its own documentation.
//!
//! So a prefixed header registers the bare credential and nothing else. Registering the *prefixed*
//! form instead would be actively worse in two ways, and both are the failure C-159 §2 found for
//! query placement, where the registered string and the travelling string diverged:
//!
//! 1. It would put `SSWS ` — a public literal — into the redactor, so the word would be scrubbed out
//!    of unrelated prose, including the error messages a human reads to fix a broken credential.
//! 2. It would leave the **bare** token unregistered, and the bare token is what a vendor's 401 body
//!    echoes back and what an operator pastes into a shell. The prefixed form is not the only form
//!    the value appears in; the bare one is.
//!
//! `a_prefixed_header_travels_prefixed_and_still_contains_the_registered_secret` below pins the
//! substring property the argument rests on.

use catalog::{Acquisition, Credential, Placement};

use crate::request::Request;
use crate::Error;

/// The value a credential contributes to a request, and where it goes.
///
/// Split from the raw secret deliberately: [`acquire`] is a pure function of the stored value, so
/// the caller can register **both** strings with the redactor before any request exists, and
/// [`place`] is then a total function that cannot fail on a value it has not seen.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Assembled {
    /// The credential's flat-namespace name, for a refusal that says which one.
    pub(crate) credential: &'static str,
    /// The acquired value, before any placement prefix.
    pub(crate) value: String,
    /// Where it goes.
    pub(crate) place: Placement,
}

/// **Hand-written, and the `value` does not print.** A derived `Debug` here would render the
/// assembled plaintext — `Bearer `'s token, or the base64 of a basic pair, which is as good as the
/// secret to anyone holding it. No call site formats one today, so nothing leaks now; a derive is a
/// foot-gun waiting for the first `{:?}` someone adds while debugging, and by then the value is on a
/// surface nobody scrubbed.
///
/// This is the posture [`connector_secrets::Secret`] already takes, down to omitting the length: a
/// length is a fingerprint. The credential's *name* and its *placement* stay, because they are what a
/// `Debug` of this type is read for — which credential, and where it was going.
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
/// suffix. It is passed in rather than read here so that this stays a pure function — the
/// environment is read once, by the caller, where a missing variable becomes a named error.
pub(crate) fn acquire(credential: &'static Credential, secret: &str, user: Option<&str>) -> String {
    match credential.acquire {
        Acquisition::Static => secret.to_string(),
        // A `None` user cannot happen — the caller resolves it whenever the acquisition is a join —
        // and composing `base64(":<secret>")` would be a plausible header the vendor rejects with a
        // 401 that says nothing. The empty string is what a vendor would see either way; the caller's
        // `MissingCredentialConfig` is what a human sees.
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
/// never leave, and [`Error::CredentialCollision`] when the operation's own emitted Flux already
/// sets the header the credential would occupy — a silent overwrite there would send a request
/// neither the module nor the connector author described.
pub(crate) fn place(
    operation: &str,
    assembled: &Assembled,
    request: &mut Request,
) -> Result<(), Error> {
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
            request.url.push_str(&query_encode(&assembled.value));
            Ok(())
        }
        Placement::Inbound => Err(Error::InboundCredential {
            operation: operation.to_owned(),
            credential: assembled.credential.to_owned(),
        }),
    }
}

/// Standard base64 with padding (RFC 4648 §4).
///
/// Hand-rolled rather than taken as a dependency, and that is a proportionate trade: this crate's
/// claim is that it links no transport and does nothing a reader has to take on trust, and 20 lines
/// of table lookup is cheaper to audit than a supply-chain edge added for one call site. It is
/// pinned against RFC 4648's own vectors in the tests below.
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
/// it is encoded rather than trusted to be URL-safe. This is deliberately **not** a general fix for
/// `zendesk-ticket-search`'s missing parameter encoding, which is a recorded intentional gap over
/// caller-supplied values in the *emitter* (C-144); widening it from here would put two different
/// encoders on one URL.
fn query_encode(value: &str) -> String {
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
        }))
    }

    /// RFC 4648 §10's test vectors. The encoder is the one piece of this module a reviewer cannot
    /// check by reading, so it is checked against the standard rather than against itself.
    #[test]
    fn base64_matches_the_rfcs_own_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        // Every 6-bit group, so the `+` and `/` end of the alphabet is covered too.
        assert_eq!(base64(&[0xfb, 0xff, 0xbf]), "+/+/");
    }

    /// **The plaintext does not print.** `Assembled` holds the assembled credential — `Bearer <t>`'s
    /// token, or the base64 of a basic pair — so a derived `Debug` would put it on any surface the
    /// first `{:?}` someone adds while debugging reaches. No call site formats one today; this is the
    /// assertion that keeps that from mattering (C-152, finding 2).
    ///
    /// What it must still print is the credential's own name and its placement: those are what a
    /// `Debug` of this type is read for, and dropping them would trade one bad posture for a useless
    /// one.
    #[test]
    fn an_assembled_credential_redacts_its_value_the_way_a_secret_does() {
        let credential = credential(
            Acquisition::Static,
            Placement::Header {
                name: "Authorization",
                prefix: "Bearer ",
            },
        );
        let assembled = Assembled {
            credential: credential.name,
            value: acquire(credential, SENTINEL, None),
            place: credential.place,
        };

        let rendered = format!("{assembled:?}");
        assert!(
            !rendered.contains(SENTINEL),
            "the assembled plaintext printed: {rendered}"
        );
        assert!(rendered.contains("<redacted>"), "{rendered}");
        assert!(rendered.contains("acme.token"), "{rendered}");
        assert!(rendered.contains("Authorization"), "{rendered}");
    }

    /// **Axis 3, header with prefix.** The bearer preset is a prefix string and nothing else — there
    /// is no `Bearer` arm anywhere in this module, which is the whole point of the design.
    #[test]
    fn a_header_placement_carries_its_prefix() {
        let mut request = request();
        let credential = credential(
            Acquisition::Static,
            Placement::Header {
                name: "Authorization",
                prefix: "Bearer ",
            },
        );
        let assembled = Assembled {
            credential: credential.name,
            value: acquire(credential, SENTINEL, None),
            place: credential.place,
        };
        place("acme-thing-show", &assembled, &mut request).expect("a header placement");

        assert_eq!(
            request.headers.get("Authorization").map(String::as_str),
            Some(format!("Bearer {SENTINEL}").as_str())
        );
    }

    /// The empty prefix is the same code path — a raw-value header such as Shopify's
    /// `X-Shopify-Access-Token`, which a flat scheme enum needs a whole variant for.
    #[test]
    fn an_empty_prefix_is_a_raw_value_header() {
        let mut request = request();
        let credential = credential(
            Acquisition::Static,
            Placement::Header {
                name: "X-Shopify-Access-Token",
                prefix: "",
            },
        );
        let assembled = Assembled {
            credential: credential.name,
            value: acquire(credential, SENTINEL, None),
            place: credential.place,
        };
        place("acme-thing-show", &assembled, &mut request).expect("a header placement");

        assert_eq!(
            request
                .headers
                .get("X-Shopify-Access-Token")
                .map(String::as_str),
            Some(SENTINEL)
        );
    }

    /// **C-184's three vendors, and the redaction property that lets them register only the bare
    /// secret.**
    ///
    /// Okta's `SSWS `, Statuspage's `OAuth ` and PagerDuty's `Token token=` are one code path here —
    /// there is no arm for any of them, which is the same claim the `Bearer` preset makes. What the
    /// assertion is really for is the second half: the travelling value **contains the secret
    /// verbatim**, so the redactor registration made on the acquisition axis already covers it. That
    /// is the property that distinguishes a placement prefix from a basic join, which transforms the
    /// secret and therefore needs a registration of its own.
    #[test]
    fn a_prefixed_header_travels_prefixed_and_still_contains_the_registered_secret() {
        for prefix in ["SSWS ", "OAuth ", "Token token="] {
            let mut request = request();
            let credential = credential(
                Acquisition::Static,
                Placement::Header {
                    name: "Authorization",
                    prefix,
                },
            );
            let value = acquire(credential, SENTINEL, None);
            let assembled = Assembled {
                credential: credential.name,
                value,
                place: credential.place,
            };
            place("acme-thing-show", &assembled, &mut request).expect("a header placement");

            let travelled = request
                .headers
                .get("Authorization")
                .expect("the credential was placed");
            assert_eq!(travelled, &format!("{prefix}{SENTINEL}"));
            // The load-bearing one: scrubbing the registered value scrubs the travelling value, so
            // registering the prefixed form would add a public word to the redactor and leave the
            // bare secret — the form a 401 body echoes — unheld. See the module docs.
            assert!(
                travelled.contains(SENTINEL),
                "a placement prefix must surround the secret, never transform it: {travelled}"
            );
            assert_eq!(
                travelled.replace(SENTINEL, "<redacted>"),
                format!("{prefix}<redacted>")
            );
        }
    }

    /// **Axis 2, basic base64.** Zendesk's shape: an env-sourced user half plus the literal `/token`
    /// marker, joined to the stored secret and encoded.
    #[test]
    fn basic_joins_the_user_half_and_base64s_the_pair() {
        let mut request = request();
        let credential = credential(
            Acquisition::BasicJoin {
                user_env: &["ACME_USER"],
                user_suffix: "/token",
            },
            Placement::Header {
                name: "Authorization",
                prefix: "Basic ",
            },
        );
        let value = acquire(credential, SENTINEL, Some("me@corp.example/token"));
        let assembled = Assembled {
            credential: credential.name,
            value: value.clone(),
            place: credential.place,
        };
        place("acme-thing-show", &assembled, &mut request).expect("a header placement");

        assert_eq!(
            value,
            base64(format!("me@corp.example/token:{SENTINEL}").as_bytes())
        );
        assert_eq!(
            request.headers.get("Authorization").map(String::as_str),
            Some(format!("Basic {value}").as_str())
        );
        // The composed blob is the thing that travels, and it is not the secret — which is why the
        // caller must register both with the redactor.
        assert!(!value.contains(SENTINEL));
    }

    /// **Axis 3, query placement** — and the separator, which is the half that goes wrong silently:
    /// a URL that already carries a query needs `&`, and `?` would produce a second query string the
    /// vendor answers by ignoring the credential.
    #[test]
    fn a_query_placement_picks_its_own_separator_and_encodes_the_value() {
        let credential = credential(Acquisition::Static, Placement::Query { name: "api_key" });
        let assembled = Assembled {
            credential: credential.name,
            value: acquire(credential, "a+b/c=d", None),
            place: credential.place,
        };

        let mut fresh = request();
        place("acme-thing-show", &assembled, &mut fresh).expect("a query placement");
        assert_eq!(
            fresh.url,
            "https://vendor.example/api/v2/things?api_key=a%2Bb%2Fc%3Dd"
        );

        let mut existing = request();
        existing.url.push_str("?page=2");
        place("acme-thing-show", &assembled, &mut existing).expect("a query placement");
        assert_eq!(
            existing.url,
            "https://vendor.example/api/v2/things?page=2&api_key=a%2Bb%2Fc%3Dd"
        );
    }

    /// A signing secret verifies bytes that arrived. Placing one on an outgoing request would hand
    /// the vendor the value that authenticates *their* calls to us, so it is refused rather than
    /// skipped — a skip would send the request unauthenticated instead.
    #[test]
    fn an_inbound_credential_is_refused_rather_than_sent() {
        let mut request = request();
        let credential = credential(Acquisition::Static, Placement::Inbound);
        let assembled = Assembled {
            credential: credential.name,
            value: SENTINEL.to_string(),
            place: credential.place,
        };

        let error = place("acme-thing-show", &assembled, &mut request)
            .expect_err("a signing secret never goes out");
        assert!(matches!(error, Error::InboundCredential { .. }), "{error}");
        assert!(request.headers.is_empty());
        assert!(!request.url.contains(SENTINEL));
    }

    /// The module's own emitted Flux owns the headers it sets. Overwriting one silently would send a
    /// request neither surface describes; the loader already refuses an authored `Authorization`, so
    /// this is the second lock rather than the first.
    #[test]
    fn a_credential_never_overwrites_a_header_the_module_set() {
        let mut request = request();
        request
            .headers
            .insert("authorization".to_string(), "already here".to_string());
        let credential = credential(
            Acquisition::Static,
            Placement::Header {
                name: "Authorization",
                prefix: "Bearer ",
            },
        );
        let assembled = Assembled {
            credential: credential.name,
            value: SENTINEL.to_string(),
            place: credential.place,
        };

        let error = place("acme-thing-show", &assembled, &mut request)
            .expect_err("the header is already taken");
        assert!(
            matches!(error, Error::CredentialCollision { .. }),
            "{error}"
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("already here")
        );
    }
}
