//! The verification conformance matrix: one parameterized HMAC, checked against real vendors.
//!
//! [`HmacSpec`] is the finding `docs/designs/inbound-events.md` rests on — that GitHub, Stripe,
//! Slack and Zendesk document four bespoke-looking webhook signatures which vary along exactly four
//! axes, so verification is a struct a compiler can carry rather than a script somebody hand-writes
//! per vendor. This file is the test of that claim, and it is the load-bearing one: if the matrix
//! does not actually reproduce a vendor's own published signature, then generated verification is
//! generated *wrongly*, and every connector that ships it presents forged events as trusted.
//!
//! # What makes this a proof rather than a tautology
//!
//! Three rules, and the file is arranged around them.
//!
//! 1. **The verifier below is parameterized only by [`HmacSpec`].** There is no `match vendor` in
//!    it, and there cannot be one: a per-vendor branch here would prove that four schemes can be
//!    verified by four implementations, which nobody doubted. Every vendor difference must enter
//!    through the declared struct or the row does not conform.
//! 2. **The specs come through the real loader.** Each row is the vendor's parameters written as a
//!    `[channels.verification.hmac]` table and passed to [`provider::load`], because "the matrix
//!    covers this vendor" means an author can *write it down* — not that the struct could hold it
//!    in principle. Slack's row is the shipped `providers/slack.toml`, read from disk.
//! 3. **The signature vectors are the vendors' own**, where the vendor publishes one. A vector this
//!    repository generated with this repository's HMAC would agree with itself no matter which
//!    bytes it signed, which is precisely the trap the story records. Provenance is stated per row
//!    in each row's [`Source`], and the two rows that are not vendor-published say so.
//!
//! The HMAC primitive underneath is pinned separately to **RFC 4231**'s published test vectors
//! ([`the_hmac_primitive_matches_rfc_4231`]). That is what makes rule 3 checkable: an implementation
//! that reproduces RFC 4231 and *also* reproduces GitHub's and Slack's documented digests from their
//! documented inputs cannot have arrived there by agreeing with itself.
//!
//! # No secret in this file is real
//!
//! Every secret here is either a vendor's own documentation placeholder — GitHub's
//! `It's a Secret to Everybody`, Slack's `8f742231b10e8888abcd99yyyzzz85a5`, which is not even hex —
//! or an unmistakable sentinel. A fixture *shaped* like a credential is its own incident: it trips
//! push protection and blocks a release without ever having been a secret.

use std::path::{Path, PathBuf};

use connector_spec::inbound::{signed_placeholders, SIGNED_PLACEHOLDERS};
use connector_spec::{provider, Digest, Encoding, FieldSource, HmacSpec, VerificationScheme};
use sha2::{Digest as _, Sha256};

// -------------------------------------------------------------------------------------------
// The reference verifier — driven by `HmacSpec` and nothing else
// -------------------------------------------------------------------------------------------

/// An inbound request as it arrives at an endpoint: raw bytes and headers, nothing parsed.
///
/// The body is `Vec<u8>` rather than `String` deliberately. Verification runs **before** parsing,
/// so the type the verifier accepts must be the type the wire delivers; a `String` here would make
/// the "re-serialize then verify" bypass representable, and that bypass is the classic defect this
/// whole design exists to prevent.
struct Request {
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    /// The verifier's "now", in unix seconds. Injected rather than read from the clock so that a
    /// tolerance test is a test and not a race.
    now: i64,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Why a request was not accepted. Every variant is a refusal; there is no "accepted with a
/// warning", because a webhook verifier that half-accepts has already delivered the event.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    /// The signature header is absent.
    MissingHeader,
    /// The header is present but not in the shape the spec declares.
    Malformed(String),
    /// The digest did not match. The common case, and the only one an attacker sees.
    Mismatch,
    /// The signed timestamp is outside the declared window.
    Stale,
    /// **The scheme is outside the matrix.** Stated explicitly and never silently: a verifier that
    /// cannot express a vendor's scheme must say so, because the alternative is a connector that
    /// looks verified and is not.
    CannotVerify(String),
}

/// Verify one inbound request against one declared scheme.
///
/// Note what this function does not contain: any mention of GitHub, Stripe, Slack or Zendesk. That
/// absence is the conformance claim.
fn verify(spec: &HmacSpec, secret: &[u8], request: &Request) -> Result<(), Refusal> {
    // Axis 1 — the digest.
    let mac = match spec.algorithm {
        Digest::Sha256 => hmac_sha256,
        // SHA-1 is in the IR only because GitHub's superseded `X-Hub-Signature` used it. Nothing
        // ships it, and a scheme that needs it gets a stated refusal rather than a quiet pass.
        Digest::Sha1 => {
            return Err(Refusal::CannotVerify(
                "sha1 signatures are not verified by this matrix".to_owned(),
            ))
        }
    };

    // Axis 2 — how the digest is spelled in the header.
    let raw = request.header(&spec.header).ok_or(Refusal::MissingHeader)?;
    let encoded = match &spec.prefix {
        Some(prefix) => raw
            .strip_prefix(prefix.as_str())
            .ok_or_else(|| Refusal::Malformed(format!("header does not start with {prefix:?}")))?,
        None => raw,
    };
    // A comma-separated key/value list is not an encoded digest, and no field of `HmacSpec` says
    // which element to take. Stripe's `t=…,v1=…` lands here. Refusing is the point: the row is
    // outside the matrix, and the manifest must say so rather than a host guessing.
    //
    // Trailing `=` is base64 padding, not an assignment, so it is trimmed before the test —
    // Zendesk's signatures end in one.
    if encoded.contains(',') || encoded.trim_end_matches('=').contains('=') {
        return Err(Refusal::CannotVerify(format!(
            "header value {encoded:?} is a key/value list, not an encoded digest; `HmacSpec` \
             declares a literal `prefix` and has no axis for selecting a component"
        )));
    }
    let provided = decode(spec.encoding, encoded)
        .map_err(|reason| Refusal::Malformed(format!("signature is not {reason}")))?;

    // Axis 3 — the string that is signed, and where its timestamp comes from.
    let timestamp = match &spec.timestamp {
        Some(selector) => match selector.source {
            FieldSource::Header => Some(
                request
                    .header(&selector.name)
                    .ok_or(Refusal::MissingHeader)?
                    .to_owned(),
            ),
            // Reading the timestamp out of the body would mean parsing the body before verifying
            // it — the exact ordering the design forbids, and a parser reachable by an unverified
            // caller.
            FieldSource::Body => {
                return Err(Refusal::CannotVerify(
                    "a body-sourced timestamp would have to be parsed before it is verified"
                        .to_owned(),
                ))
            }
        },
        None => None,
    };
    let message = signed_message(&spec.signed, &request.body, timestamp.as_deref())?;

    // Axis 4 — how long a signature stays acceptable.
    if let (Some(tolerance), Some(stamp)) = (&spec.tolerance, timestamp.as_deref()) {
        let window = parse_tolerance(tolerance).map_err(|reason| {
            Refusal::CannotVerify(format!("tolerance {tolerance:?}: {reason}"))
        })?;
        let signed_at = parse_timestamp(stamp)
            .map_err(|reason| Refusal::Malformed(format!("timestamp {stamp:?}: {reason}")))?;
        if (request.now - signed_at).abs() > window {
            return Err(Refusal::Stale);
        }
    }

    if constant_time_eq(&mac(secret, &message), &provided) {
        Ok(())
    } else {
        Err(Refusal::Mismatch)
    }
}

/// Render `template` into the exact bytes that are signed.
///
/// Byte-oriented on purpose: the body is spliced in as it arrived, never round-tripped through a
/// `String`. The template is refused outright if it interpolates anything the host cannot fill —
/// including the placeholder [`signed_placeholders`] reports for an unterminated `{`. A renderer
/// that instead emitted such a fragment as a literal would produce a signed string that does not
/// contain the body at all, and every forged payload would then verify.
fn signed_message(
    template: &str,
    body: &[u8],
    timestamp: Option<&str>,
) -> Result<Vec<u8>, Refusal> {
    for placeholder in signed_placeholders(template) {
        if !SIGNED_PLACEHOLDERS.contains(&placeholder.as_str()) {
            return Err(Refusal::CannotVerify(format!(
                "signed template {template:?} interpolates {{{placeholder}}}, which the host \
                 cannot fill"
            )));
        }
    }

    let mut out = Vec::with_capacity(template.len() + body.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.extend_from_slice(&rest.as_bytes()[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            // No closing brace, and nothing left to substitute: a host that trusted
            // `signed_placeholders` — which reports the placeholders it *could* read — appends the
            // fragment as literal text, exactly as it would any other constant in the template.
            //
            // This branch is deliberately the naive one. It is safe only because the check above
            // refuses such a template outright, and it is safe only if `signed_placeholders`
            // reports the unterminated fragment rather than silently swallowing it. If those two
            // ever disagree, the signed string loses the body and every forgery verifies — which is
            // what `assert_body_is_signed` demonstrates before demanding the refusal.
            out.push(b'{');
            out.extend_from_slice(after.as_bytes());
            return Ok(out);
        };
        match &after[..close] {
            "body" => out.extend_from_slice(body),
            "timestamp" => match timestamp {
                Some(value) => out.extend_from_slice(value.as_bytes()),
                None => {
                    return Err(Refusal::CannotVerify(
                        "the template signs a timestamp but no selector reads one".to_owned(),
                    ))
                }
            },
            other => {
                return Err(Refusal::CannotVerify(format!(
                    "signed template {template:?} interpolates {{{other}}}"
                )))
            }
        }
        rest = &after[close + 1..];
    }
    out.extend_from_slice(rest.as_bytes());
    Ok(out)
}

// -------------------------------------------------------------------------------------------
// Comparison, in constant time
// -------------------------------------------------------------------------------------------

/// Compare two digests without leaking where they first differ.
///
/// `a == b` on two `&[u8]` short-circuits: it compares lengths, then bytes, and returns at the
/// first mismatch. An attacker who can time the endpoint recovers the expected digest one byte at a
/// time from that. This folds every byte instead and returns a single accumulated answer, and
/// [`comparison_examines_every_byte_wherever_they_differ`] pins the absence of the early exit
/// mechanically rather than by review.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    compare_counting(a, b).0
}

/// [`constant_time_eq`] plus the number of byte comparisons it performed, so a test can assert the
/// count does not depend on the data. Length is deliberately folded into the result rather than
/// short-circuited on.
fn compare_counting(a: &[u8], b: &[u8]) -> (bool, usize) {
    let mut diff = (a.len() ^ b.len()) as u8;
    let mut examined = 0;
    for index in 0..a.len().max(b.len()) {
        let left = a.get(index).copied().unwrap_or(0);
        let right = b.get(index).copied().unwrap_or(0);
        diff |= left ^ right;
        examined += 1;
    }
    (diff == 0 && a.len() == b.len(), examined)
}

// -------------------------------------------------------------------------------------------
// The primitives: HMAC, hex, base64, durations, timestamps
// -------------------------------------------------------------------------------------------

/// HMAC-SHA256, RFC 2104, over the `sha2` crate this workspace already pins.
fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    const BLOCK: usize = 64;

    let mut padded = [0u8; BLOCK];
    if key.len() > BLOCK {
        padded[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        padded[..key.len()].copy_from_slice(key);
    }

    let mut inner = Sha256::new();
    inner.update(padded.iter().map(|b| b ^ 0x36).collect::<Vec<_>>());
    inner.update(message);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(padded.iter().map(|b| b ^ 0x5c).collect::<Vec<_>>());
    outer.update(inner);
    outer.finalize().to_vec()
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Decode a header's digest according to the declared encoding.
///
/// Length is not checked here. A truncated signature must fail on the *comparison*, so that the
/// negative case in [`vendor_signature_vectors_verify`] exercises the comparison rather than a
/// length guard standing in front of it.
fn decode(encoding: Encoding, value: &str) -> Result<Vec<u8>, String> {
    match encoding {
        Encoding::Hex => {
            if !value.len().is_multiple_of(2) {
                return Err("hex: odd length".to_owned());
            }
            (0..value.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| "hex".to_owned()))
                .collect()
        }
        Encoding::Base64 => {
            let mut bits = 0u32;
            let mut held = 0;
            let mut out = Vec::new();
            for ch in value.chars().filter(|c| *c != '=') {
                let Some(index) = BASE64.iter().position(|c| *c as char == ch) else {
                    return Err("base64".to_owned());
                };
                bits = (bits << 6) | index as u32;
                held += 6;
                if held >= 8 {
                    held -= 8;
                    out.push((bits >> held) as u8);
                }
            }
            Ok(out)
        }
    }
}

/// `5m`, `300s` — the spelling [`HmacSpec::tolerance`] documents, in seconds.
///
/// Nothing in the crate parses this today and the loader does not check its shape, so a
/// `tolerance = "banana"` loads and the window is whatever a host decides at runtime.
/// [`every_shipped_tolerance_is_a_window_a_host_can_actually_apply`] is the gate that keeps that
/// from shipping while the loader has no opinion.
fn parse_tolerance(tolerance: &str) -> Result<i64, String> {
    let (digits, scale) = match tolerance.strip_suffix('s') {
        Some(digits) => (digits, 1),
        None => match tolerance.strip_suffix('m') {
            Some(digits) => (digits, 60),
            None => match tolerance.strip_suffix('h') {
                Some(digits) => (digits, 3600),
                None => return Err("no unit; a window reads as `5m`, `300s` or `1h`".to_owned()),
            },
        },
    };
    digits
        .parse::<i64>()
        .map(|count| count * scale)
        .map_err(|_| format!("{digits:?} is not a whole number of units"))
}

/// A signed timestamp, in unix seconds.
///
/// **This function is a finding, not a feature.** `HmacSpec` says *where* the timestamp is read
/// from and never *how it is spelled*, so a host must sniff: Slack and Stripe send unix seconds,
/// Zendesk sends RFC 3339. Sniffing is exactly the guessing the `timestamp` selector was added to
/// stop — see the deviations recorded on the story.
fn parse_timestamp(value: &str) -> Result<i64, String> {
    if let Ok(seconds) = value.parse::<i64>() {
        return Ok(seconds);
    }
    let bytes = value.as_bytes();
    if bytes.len() != 20 || bytes[10] != b'T' || bytes[19] != b'Z' {
        return Err("neither unix seconds nor `YYYY-MM-DDTHH:MM:SSZ`".to_owned());
    }
    let number = |range: std::ops::Range<usize>| -> Result<i64, String> {
        value[range]
            .parse::<i64>()
            .map_err(|_| "not a date".to_owned())
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);

    // Days from the civil calendar to the unix epoch (Howard Hinnant's `days_from_civil`).
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;

    Ok(days * 86_400 + hour * 3600 + minute * 60 + second)
}

// -------------------------------------------------------------------------------------------
// The matrix
// -------------------------------------------------------------------------------------------

/// Where a row's signature vector came from. The distinction is the whole reason this story exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// The vendor documents this exact `(secret, body, signature)` triple. Reproducing it proves the
    /// declared parameters describe what the vendor actually sends.
    VendorDocumented,
    /// The vendor documents its **parameters** but publishes no worked triple, so the digest was
    /// computed by an implementation outside this repository (CPython's `hmac`/`hashlib`) and
    /// committed. Weaker: it proves the axes are expressible and that this implementation agrees
    /// with an independent one, not that the vendor sends these bytes.
    IndependentImplementation,
    /// The vendor's own published header, used to show the scheme does **not** fit the matrix.
    VendorHeaderOnlyCannotVerify,
}

/// One vendor's row: the parameters as an author would write them, and a request to check them on.
struct Row {
    vendor: &'static str,
    source: Source,
    /// The `[channels.verification.hmac]` table, in the vendor's own published parameters. `None`
    /// means the row is read from a shipped `providers/*.toml` instead.
    hmac_toml: Option<&'static str>,
    /// The shipped provider and channel this row is read from, when it has one.
    shipped: Option<(&'static str, &'static str)>,
    secret: &'static str,
    body: &'static [u8],
    signature: (&'static str, &'static str),
    timestamp: Option<(&'static str, &'static str)>,
    now: i64,
    /// Whether `HmacSpec` can express this vendor at all. A `false` row must produce a stated
    /// [`Refusal::CannotVerify`], never a pass.
    in_matrix: bool,
}

fn matrix() -> Vec<Row> {
    vec![
        // -- GitHub -----------------------------------------------------------------------------
        // docs.github.com "Validating webhook deliveries" publishes this triple verbatim: the
        // secret is a sentence, the payload is `Hello, World!`, and the digest is the one below.
        Row {
            vendor: "github",
            source: Source::VendorDocumented,
            hmac_toml: Some(
                r#"
[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Hub-Signature-256"
prefix = "sha256="
signed = "{body}"
secret = "acme.webhook_secret"
"#,
            ),
            shipped: None,
            secret: "It's a Secret to Everybody",
            body: b"Hello, World!",
            signature: (
                "X-Hub-Signature-256",
                "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17",
            ),
            timestamp: None,
            now: 0,
            in_matrix: true,
        },
        // -- Slack ------------------------------------------------------------------------------
        // The one row that is **shipped**: `providers/slack.toml`'s `events-api` binding, read from
        // disk rather than restated here, so this test fails if that file is edited into a shape
        // that no longer reproduces Slack's own example.
        //
        // The vector is docs.slack.dev "Verifying requests from Slack". Its example signing secret
        // contains `yyyzzz` and is therefore not even valid hex — Slack obfuscated it, and it is
        // repeated here for that reason.
        Row {
            vendor: "slack",
            source: Source::VendorDocumented,
            hmac_toml: None,
            shipped: Some(("slack", "events-api")),
            secret: "8f742231b10e8888abcd99yyyzzz85a5",
            body: SLACK_BODY,
            signature: (
                "X-Slack-Signature",
                "v0=a2114d57b48eac39b9ad189dd8316235a7b4a8d21a10bd27519666489c69b503",
            ),
            timestamp: Some(("X-Slack-Request-Timestamp", "1531420618")),
            // Inside Slack's documented five-minute window, and the stale case below steps outside
            // it.
            now: 1_531_420_618 + 60,
            in_matrix: true,
        },
        // -- Zendesk ----------------------------------------------------------------------------
        // Parameters from developer.zendesk.com "Verifying webhook authenticity": base64 rather
        // than hex, no prefix, an RFC 3339 timestamp concatenated straight onto the body. Zendesk
        // publishes no worked triple, so the digest is CPython's, not this repository's.
        Row {
            vendor: "zendesk",
            source: Source::IndependentImplementation,
            hmac_toml: Some(
                r#"
[channels.verification.hmac]
algorithm = "sha256"
encoding = "base64"
header = "X-Zendesk-Webhook-Signature"
signed = "{timestamp}{body}"
timestamp = { source = "header", name = "X-Zendesk-Webhook-Signature-Timestamp" }
secret = "acme.webhook_secret"
tolerance = "5m"
"#,
            ),
            shipped: None,
            secret: SENTINEL_SECRET,
            body: br#"{"ticket":{"id":42,"status":"open"}}"#,
            signature: (
                "X-Zendesk-Webhook-Signature",
                "imSBmAjFoQlrb7kHW9Xe96ySL8AuJ/CULYmz2ag9yls=",
            ),
            timestamp: Some((
                "X-Zendesk-Webhook-Signature-Timestamp",
                "2022-05-05T18:32:28Z",
            )),
            now: 1_651_775_548 + 60,
            in_matrix: true,
        },
        // -- Stripe -----------------------------------------------------------------------------
        // The row that does **not** fit. Stripe's signed string (`{timestamp}.{body}`) is
        // expressible; its *header* is not. `Stripe-Signature` is a comma-separated key/value list
        // carrying the timestamp and one digest per scheme version, and `HmacSpec` has one literal
        // `prefix` and a `Selector` that addresses a whole header — neither can take a component
        // out of that list. The header below is Stripe's own documented example.
        //
        // So the expectation is an explicit refusal. That is acceptance, not a gap swept under the
        // rug: a vendor outside the matrix must produce a stated "cannot verify" that a manifest can
        // publish loudly, never a scheme that looks verified and passes everything.
        Row {
            vendor: "stripe",
            source: Source::VendorHeaderOnlyCannotVerify,
            hmac_toml: Some(
                r#"
[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "Stripe-Signature"
signed = "{timestamp}.{body}"
timestamp = { source = "header", name = "Stripe-Signature" }
secret = "acme.webhook_secret"
tolerance = "5m"
"#,
            ),
            shipped: None,
            secret: SENTINEL_SECRET,
            body: br#"{"id":"evt_00000000000000","type":"payment_intent.succeeded"}"#,
            signature: (
                "Stripe-Signature",
                "t=1492774577,v1=5257a869e7ecebeda32affa62cdca3fa51cad7e77a0e56ff536d0ce8e108d8bd",
            ),
            timestamp: None,
            now: 1_492_774_577,
            in_matrix: false,
        },
    ]
}

/// Slack's documented example payload, a form-encoded slash-command body.
const SLACK_BODY: &[u8] = b"token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2FT1DC2JH3J%2F397700885554%2F96rGlfmibIGlgcZRskXaIFfN&trigger_id=398738663015.47445629121.803a0bc887a14d10d2c447fce8b6703c";

/// The secret for every row a vendor does not publish one for. Unmistakably not a credential: a
/// fixture shaped like one is an incident on its own, independent of whether it ever was a secret.
const SENTINEL_SECRET: &str = "not-a-real-secret-conformance-fixture-only";

impl Row {
    /// The declared scheme, obtained the way an author would obtain it — through the loader.
    fn spec(&self) -> HmacSpec {
        match (self.hmac_toml, self.shipped) {
            (Some(table), None) => {
                let source = fixture(table);
                let loaded =
                    provider::load("providers/fixture.toml", &source).unwrap_or_else(|error| {
                        panic!(
                            "vendor {}: its parameters must be writable in a provider file, \
                                    but the loader refused them:\n{error}",
                            self.vendor
                        )
                    });
                let Some(VerificationScheme::Hmac(spec)) = loaded
                    .connector
                    .channel("hook")
                    .expect("the fixture declares one binding")
                    .verification
                    .clone()
                else {
                    panic!(
                        "vendor {}: the binding must verify with an HMAC scheme",
                        self.vendor
                    );
                };
                spec
            }
            (None, Some((provider_id, channel))) => shipped_spec(provider_id, channel),
            _ => panic!(
                "vendor {}: a row is either written out or shipped",
                self.vendor
            ),
        }
    }

    /// A well-formed request carrying this row's known-good signature.
    fn request(&self) -> Request {
        let mut headers = vec![(self.signature.0.to_owned(), self.signature.1.to_owned())];
        if let Some((name, value)) = self.timestamp {
            headers.push((name.to_owned(), value.to_owned()));
        }
        Request {
            headers,
            body: self.body.to_vec(),
            now: self.now,
        }
    }
}

/// A connector with one event and one signing credential, ready for a `[channels.*]` block. The
/// same shape `channel_bindings.rs` uses, because the claim under test is about the file an author
/// writes.
fn fixture(hmac: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
api_version = "v1"
base_url = "https://api.acme.example"

[[auth]]
name = "acme.webhook_secret"
scheme = "signing"
env = ["ACME_WEBHOOK_SECRET"]

# A connector must describe at least one operation, so the binding has a connector to hang off.
[[operations]]
id = "acme-ping"
method = "GET"
path = "/ping"
risk = "low"
idempotency = "idempotent"

[[events]]
name = "thing.created"

[[channels]]
name = "hook"
transport = "webhook"
events = ["thing.created"]
{hmac}
[channels.setup]
steps = ["Paste the Request URL into the Acme dashboard"]
"#
    )
}

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// Every `HmacSpec` this repository actually ships, as `(provider, channel, spec)`.
///
/// Read from `providers/` rather than listed, for the reason `shipped_providers.rs` records: a list
/// and a directory drift in exactly one direction, and the direction is "a new provider is not
/// covered".
fn shipped_specs() -> Vec<(String, String, HmacSpec)> {
    let dir = providers_dir();
    let mut found = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "{} holds no provider definitions, so this gate would pass vacuously",
        dir.display()
    );

    for path in files {
        let name = path
            .file_stem()
            .expect("a stem")
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let loaded = provider::load(&format!("providers/{name}.toml"), &source)
            .unwrap_or_else(|error| panic!("providers/{name}.toml does not load: {error}"));
        for channel in &loaded.connector.channels {
            if let Some(VerificationScheme::Hmac(spec)) = &channel.verification {
                found.push((name.clone(), channel.name.clone(), spec.clone()));
            }
        }
    }
    found
}

fn shipped_spec(provider_id: &str, channel: &str) -> HmacSpec {
    shipped_specs()
        .into_iter()
        .find(|(p, c, _)| p == provider_id && c == channel)
        .unwrap_or_else(|| {
            panic!("providers/{provider_id}.toml no longer ships a `{channel}` HMAC binding")
        })
        .2
}

// -------------------------------------------------------------------------------------------
// The conformance matrix itself
// -------------------------------------------------------------------------------------------

/// **The story's failing-first test.** For every vendor in the matrix: the parameters load, the
/// signed string covers the body, the vendor's own signature verifies, and five forgeries do not.
///
/// The body-coverage assertion is the one that catches the real defect. A template is a string, and
/// a string can be wrong in a way that still parses: drop one closing brace from `{body}` and the
/// scheme signs a constant — the body is not in the signed string at all, so every payload verifies
/// against a signature captured once. That is not hypothetical, it is a one-character typo in a
/// provider file, and it must be refused at load rather than discovered in production.
#[test]
fn vendor_signature_vectors_verify() {
    for row in matrix() {
        let spec = row.spec();
        let secret = row.secret.as_bytes();
        let outcome = verify(&spec, secret, &row.request());

        if !row.in_matrix {
            // A vendor outside the matrix says so. Never a silent pass.
            let Err(Refusal::CannotVerify(reason)) = &outcome else {
                panic!(
                    "vendor {}: a scheme this matrix cannot express must produce a stated \
                     `cannot verify`, not {outcome:?}",
                    row.vendor
                );
            };
            assert!(
                !reason.is_empty(),
                "vendor {}: the refusal must carry a reason a manifest can publish",
                row.vendor
            );
            continue;
        }

        // 1. The signed string must actually cover the body.
        assert_body_is_signed(&row, &spec);

        // 2. The vendor's own vector verifies.
        assert_eq!(
            outcome,
            Ok(()),
            "vendor {}: its documented signature must verify against its documented parameters \
             ({:?} vector)",
            row.vendor,
            row.source
        );

        // 3. …and five forgeries do not.
        let tampered = {
            let mut request = row.request();
            request.body.extend_from_slice(b"!");
            request
        };
        assert_eq!(
            verify(&spec, secret, &tampered),
            Err(Refusal::Mismatch),
            "vendor {}: a mutated body must not verify",
            row.vendor
        );

        assert_eq!(
            verify(&spec, b"the-wrong-secret", &row.request()),
            Err(Refusal::Mismatch),
            "vendor {}: another party's secret must not verify",
            row.vendor
        );

        let truncated = {
            let mut request = row.request();
            let value = request.headers[0].1.clone();
            request.headers[0].1 = value[..value.len() - 4].to_owned();
            request
        };
        assert!(
            matches!(
                verify(&spec, secret, &truncated),
                Err(Refusal::Mismatch | Refusal::Malformed(_))
            ),
            "vendor {}: a truncated signature must not verify",
            row.vendor
        );

        let headerless = Request {
            headers: row.request().headers.into_iter().skip(1).collect(),
            ..row.request()
        };
        assert_eq!(
            verify(&spec, secret, &headerless),
            Err(Refusal::MissingHeader),
            "vendor {}: a request carrying no signature must not verify",
            row.vendor
        );

        let flipped = {
            let mut request = row.request();
            let mut value = request.headers[0].1.clone();
            // Flip the last character of the digest, keeping length and encoding valid, so the
            // refusal comes from the comparison and not from a decoder.
            let last = value.pop().expect("a non-empty signature");
            value.push(if last == 'a' { 'b' } else { 'a' });
            request.headers[0].1 = value;
            request
        };
        assert_eq!(
            verify(&spec, secret, &flipped),
            Err(Refusal::Mismatch),
            "vendor {}: a signature differing in one character must not verify",
            row.vendor
        );
    }
}

/// The near-miss that makes a verifier authenticate nothing: `{body}` with the closing brace
/// dropped.
///
/// It has to be refused **at load**, and this asserts why by doing the damage first — signing one
/// body and verifying a different one — before demanding the refusal. `signed_placeholders` reads a
/// template left to right and stops at an unterminated `{`; if it reports the fragment as nothing at
/// all, then a template whose *first* placeholder is well-formed passes every check the loader makes
/// and the body silently leaves the signed string.
fn assert_body_is_signed(row: &Row, spec: &HmacSpec) {
    let typo = spec.signed.replacen("{body}", "{body", 1);
    if typo == spec.signed {
        return;
    }

    let one = signed_message(&typo, b"the original payload", Some("1531420618"));
    let other = signed_message(&typo, b"a forged payload", Some("1531420618"));
    if let (Ok(one), Ok(other)) = (&one, &other) {
        assert_ne!(
            one, other,
            "vendor {}: `signed = {typo:?}` — one missing brace — produces the SAME signed string \
             for two different bodies, so a signature captured from one delivery verifies a FORGED \
             body. A tampered payload is accepted.",
            row.vendor
        );
    }

    // And the file that could contain that typo must not load in the first place.
    if let Some(table) = row.hmac_toml {
        let source = fixture(&table.replace("{body}", "{body"));
        let error = provider::load("providers/fixture.toml", &source)
            .err()
            .unwrap_or_else(|| {
                panic!(
                    "vendor {}: a `signed` template with an unterminated placeholder must be \
                     refused at load — it signs a string that does not contain the body, and every \
                     forged payload then verifies",
                    row.vendor
                )
            });
        assert!(
            format!("{error}").contains("signed"),
            "vendor {}: the refusal must name the template it rejected:\n{error}",
            row.vendor
        );
    }
}

/// The primitive, pinned to an authority outside this repository.
///
/// RFC 4231 §4.2 and §4.3. Without this the vendor rows above would only show that this file agrees
/// with itself; with it, reproducing GitHub's and Slack's published digests from their published
/// inputs is evidence the vectors are genuinely theirs.
#[test]
fn the_hmac_primitive_matches_rfc_4231() {
    assert_eq!(
        hex_encode(&hmac_sha256(&[0x0b; 20], b"Hi There")),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
        "RFC 4231 test case 1"
    );
    assert_eq!(
        hex_encode(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
        "RFC 4231 test case 2"
    );
    // A key longer than the 64-byte block is hashed first; getting this wrong is invisible until a
    // vendor issues a long secret.
    assert_eq!(
        hex_encode(&hmac_sha256(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First"
        )),
        "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
        "RFC 4231 test case 6"
    );
}

/// Re-serializing the body breaks verification — and that is the behaviour we want.
///
/// Verification runs over the bytes that arrived. A host that parses JSON and re-encodes it before
/// signing produces a different string for a semantically identical document, so key order and
/// whitespace decide whether a real event is accepted. Any "normalize, then verify" step is a
/// bypass, not a convenience: it lets an attacker choose a form of the document that normalizes to
/// something the signer never saw.
#[test]
fn reserializing_the_json_body_breaks_verification() {
    let row = matrix()
        .into_iter()
        .find(|row| row.vendor == "zendesk")
        .expect("the matrix carries a JSON-bodied row");
    let spec = row.spec();

    assert_eq!(
        verify(&spec, row.secret.as_bytes(), &row.request()),
        Ok(()),
        "the untouched bytes verify"
    );

    for reserialized in [
        // Key order changed by a round trip through a hash map.
        br#"{"ticket":{"status":"open","id":42}}"#.to_vec(),
        // Pretty-printed by a re-encoder.
        br#"{"ticket": {"id": 42, "status": "open"}}"#.to_vec(),
        // A trailing newline an encoder added.
        b"{\"ticket\":{\"id\":42,\"status\":\"open\"}}\n".to_vec(),
    ] {
        let request = Request {
            body: reserialized.clone(),
            ..row.request()
        };
        assert_eq!(
            verify(&spec, row.secret.as_bytes(), &request),
            Err(Refusal::Mismatch),
            "a re-serialized body must not verify: {}",
            String::from_utf8_lossy(&reserialized)
        );
    }
}

/// A replay window nobody tests is a replay window that does not exist.
#[test]
fn a_signature_outside_its_window_is_refused() {
    for row in matrix() {
        let spec = row.spec();
        let Some(tolerance) = spec.tolerance.as_deref() else {
            continue;
        };
        if !row.in_matrix {
            continue;
        }
        let window = parse_tolerance(tolerance).expect("a shipped window parses");

        // One second inside the window still verifies…
        let fresh = Request {
            now: signed_at(&row) + window - 1,
            ..row.request()
        };
        assert_eq!(
            verify(&spec, row.secret.as_bytes(), &fresh),
            Ok(()),
            "vendor {}: a request at the edge of the window is still current",
            row.vendor
        );

        // …one second past it does not, even though the signature is perfectly valid.
        let stale = Request {
            now: signed_at(&row) + window + 1,
            ..row.request()
        };
        assert_eq!(
            verify(&spec, row.secret.as_bytes(), &stale),
            Err(Refusal::Stale),
            "vendor {}: a captured signature must stop being accepted after {tolerance}",
            row.vendor
        );

        // And a replay from the future is the same attack with the clock the other way round.
        let ahead = Request {
            now: signed_at(&row) - window - 1,
            ..row.request()
        };
        assert_eq!(
            verify(&spec, row.secret.as_bytes(), &ahead),
            Err(Refusal::Stale),
            "vendor {}: the window is bounded in both directions",
            row.vendor
        );
    }
}

fn signed_at(row: &Row) -> i64 {
    let (_, value) = row.timestamp.expect("a timestamped row");
    parse_timestamp(value).expect("the fixture's timestamp parses")
}

/// The defect this exists to prevent is `expected == actual` on the digest.
///
/// Slice equality in Rust short-circuits, so the time to reject leaks the length of the matching
/// prefix and an attacker recovers the expected digest byte by byte. Asserting "it is constant
/// time" by measurement would be a flaky test measuring the machine; asserting that the comparison
/// performs the same number of byte operations wherever the inputs differ pins the property that
/// actually matters — the absence of an early exit — deterministically.
#[test]
fn comparison_examines_every_byte_wherever_they_differ() {
    let expected = [0x5au8; 32];

    let mut first = expected;
    first[0] ^= 0xff;
    let mut last = expected;
    last[31] ^= 0xff;

    let (equal, baseline) = compare_counting(&expected, &expected);
    assert!(equal);
    let (differs_first, count_first) = compare_counting(&expected, &first);
    let (differs_last, count_last) = compare_counting(&expected, &last);

    assert!(!differs_first && !differs_last, "both must be rejected");
    assert_eq!(
        (count_first, count_last),
        (baseline, baseline),
        "the comparison must examine every byte whether the digests differ at the first byte or \
         the last; a differing count is an early exit, and an early exit is a timing oracle"
    );

    // Behavioural agreement with the naive comparison, so constant time is not bought with
    // wrongness.
    for (a, b) in [
        (&b""[..], &b""[..]),
        (&b"a"[..], &b""[..]),
        (&b""[..], &b"a"[..]),
        (&b"abc"[..], &b"abc"[..]),
        (&b"abc"[..], &b"abd"[..]),
        (&b"abc"[..], &b"abcd"[..]),
        // A length mismatch must not be decidable by the zero padding the fold uses.
        (&b"a\0"[..], &b"a"[..]),
    ] {
        assert_eq!(
            constant_time_eq(a, b),
            a == b,
            "constant-time comparison must agree with `==` on {a:?} vs {b:?}"
        );
    }
}

/// Every HMAC scheme this repository actually ships is one this matrix has checked.
///
/// The gate that keeps the file honest as providers land: a new binding with an unverified scheme
/// fails here rather than shipping a connector whose verification nobody ever reproduced.
#[test]
fn every_shipped_hmac_scheme_is_covered_by_the_matrix() {
    let shipped = shipped_specs();
    assert!(
        !shipped.is_empty(),
        "no provider ships an HMAC binding, so this matrix would pass vacuously"
    );

    let rows = matrix();
    for (provider_id, channel, spec) in shipped {
        let covered = rows.iter().any(|row| {
            let candidate = row.spec();
            candidate.algorithm == spec.algorithm
                && candidate.encoding == spec.encoding
                && candidate.header == spec.header
                && candidate.prefix == spec.prefix
                && candidate.signed == spec.signed
        });
        assert!(
            covered,
            "providers/{provider_id}.toml's `{channel}` binding declares a scheme \
             (algorithm {:?}, encoding {:?}, header {:?}, signed {:?}) that no row of this matrix \
             checks against a vendor vector",
            spec.algorithm, spec.encoding, spec.header, spec.signed
        );
    }
}

/// A window a host cannot parse is a window it will not apply.
///
/// The loader requires a `tolerance` on any timestamped scheme but says nothing about its shape, so
/// `tolerance = "banana"` loads today. Until it has an opinion, this is the gate.
#[test]
fn every_shipped_tolerance_is_a_window_a_host_can_actually_apply() {
    for (provider_id, channel, spec) in shipped_specs() {
        let Some(tolerance) = spec.tolerance.as_deref() else {
            continue;
        };
        let window = parse_tolerance(tolerance).unwrap_or_else(|reason| {
            panic!(
                "providers/{provider_id}.toml's `{channel}` binding declares \
                 `tolerance = {tolerance:?}`, which no host can turn into a window: {reason}"
            )
        });
        assert!(
            (1..=3600).contains(&window),
            "providers/{provider_id}.toml's `{channel}` binding allows a signature to be replayed \
             for {window}s; a webhook window is minutes, not hours"
        );
    }
}

/// A vendor that publishes no signature says so, and the loader will not let it stay quiet.
///
/// The tri-state on `verification` is the whole mechanism: unset is a load error on a webhook,
/// `"none"` is a stated position a manifest can publish, and only an `hmac` table is verification.
/// Silence must never read as trust.
#[test]
fn a_transport_outside_the_matrix_declares_that_it_cannot_verify() {
    let unverifiable = fixture("verification = \"none\"\n");
    let connector = provider::load("providers/fixture.toml", &unverifiable)
        .expect("a deliberate `none` loads")
        .connector;
    assert_eq!(
        connector.channel("hook").expect("loads").verification,
        Some(VerificationScheme::None),
        "`none` is a position the manifest publishes, not an absence"
    );

    let silent = fixture("");
    let error = provider::load("providers/fixture.toml", &silent)
        .expect_err("an open endpoint that states nothing must be refused");
    assert!(
        format!("{error}").contains("states no `verification`"),
        "silence on a webhook must be a load error, never a default:\n{error}"
    );
}
