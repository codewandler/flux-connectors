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

use connector_spec::inbound::{
    parse_tolerance, signed_placeholders, PAYLOAD_PLACEHOLDERS, SIGNED_PLACEHOLDERS,
};
use connector_spec::{
    provider, Digest, Encoding, FieldSource, HmacSpec, TimestampFormat, VerificationScheme,
};
use sha2::{Digest as _, Sha256};

use crate::shipped_provider;

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
    /// The **full request URL the vendor was configured with**, which Twilio's scheme signs.
    ///
    /// Not a fact this repository ships — `ChannelBinding` deliberately carries no URL, because the
    /// endpoint address is the operator's deployment detail. It is a fact the *transport* supplies
    /// at request time, exactly as the body and the headers are, which is why it enters here and
    /// not into the IR.
    ///
    /// It is also the axis with a deployment hazard worth naming: behind a proxy or a load balancer
    /// the host sees a rewritten scheme, authority or path, and the signature was computed over the
    /// one the vendor was told about. A host that signs what it received rather than what was
    /// configured rejects every genuine delivery.
    url: String,
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
    // SHA-1 used to be a stated refusal here — it was in the IR only because GitHub's superseded
    // `X-Hub-Signature` used it, and nothing shipped it. Twilio ships it: `X-Twilio-Signature` is
    // base64(HMAC-SHA1(…)) and the vendor publishes no SHA-256 alternative. A refusal would have
    // meant declaring a binding this matrix never reproduced, which is the one outcome the story
    // forbids, so the primitive is implemented and pinned to RFC 2202 rather than the row being
    // waved through.
    let mac = match spec.algorithm {
        Digest::Sha256 => hmac_sha256,
        Digest::Sha1 => hmac_sha1,
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
    let message = signed_message(
        &spec.signed,
        &request.body,
        timestamp.as_deref(),
        Some(request.url.as_str()),
    )?;

    // Axis 4 — how long a signature stays acceptable, and axis 5 — how the timestamp is spelled.
    if let (Some(tolerance), Some(stamp)) = (&spec.tolerance, timestamp.as_deref()) {
        let window = parse_tolerance(tolerance).map_err(|reason| {
            Refusal::CannotVerify(format!("tolerance {tolerance:?}: {reason}"))
        })?;
        let signed_at = parse_timestamp(stamp, spec.timestamp_format.unwrap_or_default())
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
    url: Option<&str>,
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
            // The one placeholder that is a *derivation* rather than a splice. See
            // `reassemble_sorted_form` for why that is still a declaration and not a script.
            "sorted_form" => out.extend_from_slice(&reassemble_sorted_form(body)?),
            "url" => {
                match url {
                    Some(value) => out.extend_from_slice(value.as_bytes()),
                    None => return Err(Refusal::CannotVerify(
                        "the template signs the request URL, which this transport does not supply"
                            .to_owned(),
                    )),
                }
            }
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

/// `{sorted_form}` — the form body's fields, decoded, sorted by name, and re-joined name-then-value
/// with no separator anywhere.
///
/// # Why this is a declaration and not an escape hatch
///
/// It is the one placeholder whose value is not a splice of something the transport handed over,
/// which is exactly the objection `providers/twilio.toml` recorded against ever adding it: "a
/// `signed` template concatenates fixed strings around two named values; it cannot re-sort N form
/// fields whose count and names vary per delivery." That is true of the *template*, and it stays
/// true — the template still cannot sort anything. What sorts is this function, once, for every
/// vendor that names the placeholder. The template says *which* derivation, not *how*, and the set
/// of derivations it may name is closed by `SIGNED_PLACEHOLDERS`.
///
/// # Every step here is forced by Twilio's own published vector
///
/// - **Decoded, not raw.** Twilio's example signs `To` as `+18005551212`, which travels on the wire
///   as `To=%2B18005551212`. Splicing raw bytes reproduces nothing.
/// - **Sorted by the decoded name**, byte-wise. Twilio sorts the parameter names as strings; UTF-8
///   byte order and code-point order agree, so the two never diverge.
/// - **No delimiter**, between a name and its value or between pairs. `CallSidCA1234…Caller+1415…`.
///
/// # A repeated name is refused, not resolved
///
/// `a=1&a=2` has no defined answer: Twilio's own helper libraries build a map, so one value wins and
/// which one depends on the language. A verifier that picks silently disagrees with some other
/// correct implementation, and disagreement on an authentication path means one side accepts what
/// the other rejects. It is refused instead — a stated `cannot verify`, which is a 4xx and no
/// delivery, rather than a coin flip. Nothing Twilio sends repeats a name; a forger controls the
/// body and would.
fn reassemble_sorted_form(body: &[u8]) -> Result<Vec<u8>, Refusal> {
    let mut fields: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    if !body.is_empty() {
        for pair in body.split(|byte| *byte == b'&') {
            let (name, value) = match pair.iter().position(|byte| *byte == b'=') {
                Some(split) => (&pair[..split], &pair[split + 1..]),
                // A bare `flag` with no `=`. Twilio never sends one; decoding it as an empty value
                // is what every form parser does, and it stays deterministic either way.
                None => (pair, &pair[pair.len()..]),
            };
            fields.push((form_decode(name)?, form_decode(value)?));
        }
    }

    fields.sort_by(|left, right| left.0.cmp(&right.0));
    for pair in fields.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(Refusal::CannotVerify(format!(
                "form field {:?} appears more than once, and the reassembled order is then \
                 undefined; a verifier that guessed would disagree with another correct one",
                String::from_utf8_lossy(&pair[0].0)
            )));
        }
    }

    let mut out = Vec::new();
    for (name, value) in fields {
        out.extend_from_slice(&name);
        out.extend_from_slice(&value);
    }
    Ok(out)
}

/// One `application/x-www-form-urlencoded` component, decoded to bytes.
///
/// Bytes rather than `String`: a percent escape can denote any octet, and refusing a delivery
/// because its decoded form is not UTF-8 would be this function inventing a rule the vendor never
/// stated. The digest is over bytes regardless.
fn form_decode(component: &[u8]) -> Result<Vec<u8>, Refusal> {
    let mut out = Vec::with_capacity(component.len());
    let mut index = 0;
    while index < component.len() {
        match component[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' => {
                let hex = component
                    .get(index + 1..index + 3)
                    .and_then(|pair| std::str::from_utf8(pair).ok())
                    .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                    .ok_or_else(|| {
                        Refusal::Malformed(
                            "the form body has a truncated or non-hex percent escape".to_owned(),
                        )
                    })?;
                out.push(hex);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
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

/// HMAC-SHA1, RFC 2104, over the SHA-1 below.
///
/// Same shape as [`hmac_sha256`], different block output. It exists because Twilio signs with SHA-1
/// and publishes no alternative, and [`the_hmac_sha1_primitive_matches_rfc_2202`] pins it to
/// vectors from outside this repository — without which reproducing Twilio's published signature
/// would only show this file agreeing with itself.
fn hmac_sha1(key: &[u8], message: &[u8]) -> Vec<u8> {
    const BLOCK: usize = 64;

    let mut padded = [0u8; BLOCK];
    if key.len() > BLOCK {
        padded[..20].copy_from_slice(&sha1(key));
    } else {
        padded[..key.len()].copy_from_slice(key);
    }

    let mut inner = padded.iter().map(|b| b ^ 0x36).collect::<Vec<_>>();
    inner.extend_from_slice(message);
    let inner = sha1(&inner);

    let mut outer = padded.iter().map(|b| b ^ 0x5c).collect::<Vec<_>>();
    outer.extend_from_slice(&inner);
    sha1(&outer).to_vec()
}

/// SHA-1, FIPS 180-4 §6.1, written out here rather than taken as a dependency.
///
/// The workspace pins `sha2` and no SHA-1 crate, and a dependency list is not this story's to edit.
/// SHA-1 is ninety lines of shifts with a published test suite, so writing it costs less than the
/// manifest change and is pinned to the same standard the crate would be.
///
/// **This is a test fixture, not a recommendation.** SHA-1 is collision-broken; it is here because
/// `X-Twilio-Signature` uses it, and HMAC-SHA1's security does not rest on collision resistance.
fn sha1(message: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [
        0x6745_2301,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];

    let mut padded = message.to_vec();
    let bits = (message.len() as u64) * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bits.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (index, word) in chunk.chunks_exact(4).enumerate() {
            w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..80 {
            w[index] = (w[index - 3] ^ w[index - 8] ^ w[index - 14] ^ w[index - 16]).rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (index, word) in w.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | (!b & d), 0x5a82_7999_u32),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        for (slot, value) in state.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(value);
        }
    }

    let mut out = [0u8; 20];
    for (index, word) in state.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
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

/// The inverse of [`decode`]'s base64 arm, so a test can spell a digest the way a base64 vendor
/// sends it. Only a test builds signatures; nothing shipped encodes one.
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for group in bytes.chunks(3) {
        let mut block = [0u8; 3];
        block[..group.len()].copy_from_slice(group);
        let packed = u32::from_be_bytes([0, block[0], block[1], block[2]]);
        for slot in 0..4 {
            if slot <= group.len() {
                out.push(BASE64[(packed >> (18 - 6 * slot)) as usize & 0x3f] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

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

/// A signed timestamp, in unix seconds, read the way the scheme **declares** it is spelled.
///
/// This function used to be a finding rather than a feature: `HmacSpec` said *where* the timestamp
/// was read from and never *how it was spelled*, so it sniffed — try an integer, fall back to a date
/// shape. Sniffing was exactly the guessing the `timestamp` selector had been added to stop, and it
/// is not harmless: `20220505183228` is a plausible spelling that parses as an integer and lands
/// 600,000 years from now, so a sniffing host applies its window to a number the vendor never meant.
/// C-141 added [`TimestampFormat`], and the sniff is gone: the format is a parameter, and a value in
/// the wrong spelling is refused rather than reinterpreted.
fn parse_timestamp(value: &str, format: TimestampFormat) -> Result<i64, String> {
    match format {
        TimestampFormat::UnixSeconds => {
            return value
                .parse::<i64>()
                .map_err(|_| format!("{value:?} is not a whole number of unix seconds"))
        }
        TimestampFormat::Rfc3339 => {}
    }

    let bytes = value.as_bytes();
    if bytes.len() != 20 || bytes[10] != b'T' || bytes[19] != b'Z' {
        return Err(format!("{value:?} is not `YYYY-MM-DDTHH:MM:SSZ`"));
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
    /// The URL the vendor was configured with, for a row whose scheme signs it. Every other row
    /// leaves it unset, and an unset URL that a template then names is a stated refusal rather than
    /// an empty string quietly signed.
    url: Option<&'static str>,
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
            url: None,
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
            url: None,
            // Inside Slack's documented five-minute window, and the stale case below steps outside
            // it.
            now: 1_531_420_618 + 60,
            in_matrix: true,
        },
        // -- Zendesk ----------------------------------------------------------------------------
        // Parameters from developer.zendesk.com "Verifying webhook authenticity": base64 rather
        // than hex, no prefix, an RFC 3339 timestamp concatenated straight onto the body. Zendesk
        // publishes no worked triple, so the digest is CPython's, not this repository's.
        //
        // The row that motivates `timestamp_format`: it is the one vendor here that does not send
        // unix seconds, and before C-141 the verifier had to sniff the spelling to read it.
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
timestamp_format = "rfc3339"
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
            url: None,
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
            url: None,
            now: 1_492_774_577,
            in_matrix: false,
        },
        // -- Twilio -----------------------------------------------------------------------------
        // The row this story exists for, and the second **shipped** one: it is read from
        // `providers/twilio.toml`'s `message-status-callback` binding rather than restated here.
        //
        // Twilio publishes the whole worked example at twilio.com/docs/usage/security — the URL,
        // the five parameters, the auth token `12345`, the exact concatenated string, and the
        // signature below. Everything about it is outside the matrix's previous reach: SHA-1 rather
        // than SHA-256, base64, no timestamp and therefore no window, and a signed string that
        // contains neither the raw body nor any constant this repository could have guessed.
        //
        // The body here is the wire form of that example — note `%2B` where the signed string has a
        // literal `+`. That single detail is the proof that `{sorted_form}` is a derivation and not
        // `{body}` under another name: splice the raw bytes and Twilio's own signature does not
        // reproduce. `the_reassembled_form_is_a_derivation_of_the_body_and_not_its_bytes` pins it.
        Row {
            vendor: "twilio",
            source: Source::VendorDocumented,
            hmac_toml: None,
            shipped: Some(("twilio", "message-status-callback")),
            secret: "12345",
            body: TWILIO_BODY,
            signature: ("X-Twilio-Signature", "L/OH5YylLD5NRKLltdqwSvS0BnU="),
            timestamp: None,
            url: Some("https://example.com/myapp.php?foo=1&bar=2"),
            now: 0,
            in_matrix: true,
        },
    ]
}

/// Slack's documented example payload, a form-encoded slash-command body.
const SLACK_BODY: &[u8] = b"token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2FT1DC2JH3J%2F397700885554%2F96rGlfmibIGlgcZRskXaIFfN&trigger_id=398738663015.47445629121.803a0bc887a14d10d2c447fce8b6703c";

/// Twilio's documented example parameters, as the form body that carries them on the wire.
///
/// Declaration order is deliberately *not* sorted: `To` first, `CallSid` last. The scheme sorts, and
/// a fixture already in sorted order would let a verifier that skipped the sort still pass.
const TWILIO_BODY: &[u8] =
    b"To=%2B18005551212&From=%2B14158675310&Digits=1234&Caller=%2B14158675310&CallSid=CA1234567890ABCDE";

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
            url: self.url.unwrap_or_default().to_owned(),
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
direction = "read"
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
        let loaded = shipped_provider::load_definition(&name, &source)
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
    // Whichever placeholder puts the payload in — `{body}` for four vendors, `{sorted_form}` for
    // Twilio. The typo is the same typo and the hole it opens is the same hole, so the check has to
    // follow the payload rather than the spelling.
    let Some(placeholder) = PAYLOAD_PLACEHOLDERS
        .iter()
        .map(|name| format!("{{{name}}}"))
        .find(|placeholder| spec.signed.contains(placeholder.as_str()))
    else {
        panic!(
            "vendor {}: `signed = {:?}` names no payload placeholder at all, which the loader must \
             already have refused",
            row.vendor, spec.signed
        );
    };
    let typo = spec
        .signed
        .replacen(&placeholder, placeholder.trim_end_matches('}'), 1);

    let url = row.url;
    let one = signed_message(&typo, b"a=the+original+payload", Some("1531420618"), url);
    let other = signed_message(&typo, b"a=a+forged+payload", Some("1531420618"), url);
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
        let source = fixture(&table.replace(&placeholder, placeholder.trim_end_matches('}')));
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

/// **The same defect as the missing brace, reachable with no typo at all.**
///
/// `signed = "{timestamp}"` is a well-formed template. Every check the loader made before C-141
/// passed on it: the placeholder list is non-empty, every name is fillable, the timestamp selector is
/// present and the tolerance is present. What it signs is a string that does not contain the body, so
/// **one captured signature verifies every forged payload for the whole window** — the tolerance is
/// the only thing bounding the attack, and it bounds it to five minutes rather than to nothing.
///
/// The forgery is demonstrated before the refusal is demanded, and it is demonstrated on the
/// **shipped** Slack parameters with `{body}` deleted from the template and nothing else touched, so
/// the reader can see that the distance between a working scheme and a forging one is one word in one
/// field. The first half keeps running after the loader learns to refuse the declaration: it is the
/// reason for the refusal, and a test that asserted only "the loader says no" would not record it.
#[test]
fn a_signed_template_that_omits_the_body_verifies_a_forged_payload() {
    // Slack's shipped binding, loaded from `providers/slack.toml`, with the body dropped out of the
    // signed string. Everything else — the digest, the hex encoding, the `v0=` prefix, the timestamp
    // header, the five-minute window — is the vendor's own and unchanged.
    let mut spec = shipped_spec("slack", "events-api");
    spec.signed = spec.signed.replace("{body}", "");
    assert_eq!(
        spec.signed, "v0:{timestamp}:",
        "the template under test is Slack's with `{{body}}` removed"
    );

    let secret = SENTINEL_SECRET.as_bytes();
    let stamp = "1531420618";
    let now = 1_531_420_618 + 60;
    let genuine = b"payload=the+delivery+the+vendor+actually+signed";
    let forged = b"payload=a+payload+the+vendor+never+saw";

    // 1. The signed string is the same for both bodies. Nothing about the payload enters it.
    let over_genuine = signed_message(&spec.signed, genuine, Some(stamp), None).expect("renders");
    let over_forged = signed_message(&spec.signed, forged, Some(stamp), None).expect("renders");
    assert_eq!(
        over_genuine, over_forged,
        "`signed = {:?}` signs a body-independent string",
        spec.signed
    );

    // 2. Capture the signature the vendor would send with the genuine delivery.
    let captured = format!("v0={}", hex_encode(&hmac_sha256(secret, &over_genuine)),);
    let request = |body: &[u8]| Request {
        headers: vec![
            (spec.header.clone(), captured.clone()),
            (
                spec.timestamp
                    .as_ref()
                    .expect("Slack's binding reads a timestamp header")
                    .name
                    .clone(),
                stamp.to_owned(),
            ),
        ],
        body: body.to_vec(),
        // Slack's template names no `{url}`, so nothing reads this.
        url: String::new(),
        now,
    };

    // 3. It verifies the genuine delivery, as it must…
    assert_eq!(
        verify(&spec, secret, &request(genuine)),
        Ok(()),
        "the captured signature verifies the delivery it was captured from"
    );
    // …and it verifies a body the holder of the secret never signed. This is the forgery: an
    // attacker who observed one delivery can now submit any payload at all for five minutes.
    assert_eq!(
        verify(&spec, secret, &request(forged)),
        Ok(()),
        "THE FORGERY: `signed = {:?}` accepts a forged body under a signature captured from a \
         different one. Verification proves only that somebody, once, held the secret",
        spec.signed
    );

    // 4. So the declaration must not load. A build is the last place this can be caught: after it,
    //    the scheme looks verified in the manifest, in the catalogue and to an operator.
    let bodyless = fixture(
        r#"
[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Acme-Signature"
prefix = "v0="
signed = "v0:{timestamp}:"
timestamp = { source = "header", name = "X-Acme-Timestamp" }
secret = "acme.webhook_secret"
tolerance = "5m"
"#,
    );
    let error = provider::load("providers/fixture.toml", &bodyless)
        .err()
        .unwrap_or_else(|| {
            panic!(
                "a `signed` template that never interpolates {{body}} must be refused at load — the \
                 forgery above is what it ships"
            )
        });
    let error = format!("{error}");
    assert!(
        error.contains("signed") && error.contains("{body}"),
        "the refusal must name the template and what it is missing:\n{error}"
    );
}

/// **C-141's hole, reopened by the new placeholders and closed again.**
///
/// Widening `SIGNED_PLACEHOLDERS` from two names to four is the moment a rule spelled "`signed` must
/// interpolate `{body}`" stops being the rule that was meant. `{url}` is a **constant per endpoint**
/// — every delivery to one callback URL signs the same string — so `signed = "{url}"` is
/// `signed = "{timestamp}"` with a longer constant and *no* window at all to bound the replay. It
/// would have loaded the moment `{url}` became fillable, and it would have read as a scheme that
/// signs something request-specific.
///
/// So the loader's rule is not about `{body}`. It is that the template must interpolate one of
/// [`PAYLOAD_PLACEHOLDERS`] — the placeholders through which the payload actually enters the signed
/// string. The forgery is demonstrated first, on Twilio's *shipped* parameters with the form
/// dropped out of the template and nothing else touched.
#[test]
fn a_signed_template_that_covers_only_the_url_verifies_a_forged_payload() {
    let mut spec = shipped_spec("twilio", "message-status-callback");
    spec.signed = spec.signed.replace("{sorted_form}", "");
    assert_eq!(
        spec.signed, "{url}",
        "the template under test is Twilio's with `{{sorted_form}}` removed"
    );

    let secret = SENTINEL_SECRET.as_bytes();
    let url = "https://hooks.example.com/twilio/status";
    let genuine = b"MessageSid=SM00000000000000000000000000000001&MessageStatus=delivered";
    let forged = b"MessageSid=SM00000000000000000000000000000002&MessageStatus=failed";

    // 1. The signed string is the same for both bodies. Nothing about the payload enters it, and
    //    unlike the timestamp case it does not even change between deliveries.
    let over_genuine = signed_message(&spec.signed, genuine, None, Some(url)).expect("renders");
    let over_forged = signed_message(&spec.signed, forged, None, Some(url)).expect("renders");
    assert_eq!(
        over_genuine, over_forged,
        "`signed = {:?}` signs a body-independent string",
        spec.signed
    );

    // 2. Capture the signature the vendor would send with the genuine delivery.
    let captured = base64_encode(&hmac_sha1(secret, &over_genuine));
    let request = |body: &[u8]| Request {
        headers: vec![(spec.header.clone(), captured.clone())],
        body: body.to_vec(),
        url: url.to_owned(),
        now: 0,
    };

    assert_eq!(
        verify(&spec, secret, &request(genuine)),
        Ok(()),
        "the captured signature verifies the delivery it was captured from"
    );
    // …and a payload the holder of the secret never signed. Twilio's scheme carries no timestamp,
    // so there is no `tolerance` to bound this: the captured signature works forever.
    assert_eq!(
        verify(&spec, secret, &request(forged)),
        Ok(()),
        "THE FORGERY: `signed = {:?}` accepts a forged body under a signature captured from a \
         different one — and with no timestamp in the scheme, for as long as the endpoint exists",
        spec.signed
    );

    // 3. So the declaration must not load.
    for template in [
        // The URL alone: a per-endpoint constant.
        r#"signed = "{url}""#,
        // And a timestamp does not rescue it — that combination is C-141's exact case with a
        // second constant bolted on.
        r#"signed = "{url}{timestamp}"
timestamp = { source = "header", name = "X-Acme-Timestamp" }
tolerance = "5m""#,
    ] {
        let source = fixture(&format!(
            r#"
[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Acme-Signature"
secret = "acme.webhook_secret"
{template}
"#
        ));
        let error = provider::load("providers/fixture.toml", &source)
            .err()
            .unwrap_or_else(|| {
                panic!(
                    "a `signed` template that never interpolates the payload must be refused at \
                     load — the forgery above is what it ships:\n{template}"
                )
            });
        let error = format!("{error}");
        assert!(
            error.contains("signed") && error.contains("{body}") && error.contains("{sorted_form}"),
            "the refusal must name the template and both ways of covering the payload:\n{error}"
        );
    }
}

/// `{sorted_form}` is a **derivation of** the body, not a second spelling of its bytes.
///
/// The distinction is the whole reason the placeholder had to be added rather than Twilio being
/// declared with `{url}{body}`, and Twilio's own vector is what settles it: the example signs
/// `To+18005551212`, while the wire carries `To=%2B18005551212`. Sorting matters too — the fixture
/// is deliberately in declaration order, not sorted order.
#[test]
fn the_reassembled_form_is_a_derivation_of_the_body_and_not_its_bytes() {
    let reassembled = reassemble_sorted_form(TWILIO_BODY).expect("Twilio's example reassembles");
    assert_eq!(
        String::from_utf8_lossy(&reassembled),
        "CallSidCA1234567890ABCDECaller+14158675310Digits1234From+14158675310To+18005551212",
        "the derivation is: percent-decode, sort by name, join name-then-value with no separator"
    );
    assert_ne!(
        reassembled,
        TWILIO_BODY.to_vec(),
        "if the reassembled form were the raw bytes, `{{body}}` would have covered Twilio and this \
         story would not exist"
    );

    // Reordering the fields on the wire must not change what is signed — that is what "sorted"
    // buys, and it is why Twilio can sign a form whose transmission order it does not control.
    let shuffled = b"CallSid=CA1234567890ABCDE&To=%2B18005551212&Caller=%2B14158675310&From=%2B14158675310&Digits=1234";
    assert_eq!(
        reassemble_sorted_form(shuffled).expect("reassembles"),
        reassembled,
        "the reassembled form is order-independent"
    );

    // A repeated name has no defined answer, so it is refused rather than resolved. A forger picks
    // the body, and two correct implementations would disagree about which value wins.
    let repeated = b"Digits=1234&Digits=9999";
    assert!(
        matches!(
            reassemble_sorted_form(repeated),
            Err(Refusal::CannotVerify(_))
        ),
        "a repeated form field name must be a stated refusal, not a coin flip"
    );
}

/// HMAC-SHA1, pinned to RFC 2202 §3 — the same role RFC 4231 plays for SHA-256.
///
/// Twilio publishes a worked triple, so its row is already strong evidence; this is what makes the
/// hand-written SHA-1 underneath it accountable to something other than itself.
#[test]
fn the_hmac_sha1_primitive_matches_rfc_2202() {
    // §3, test case 1.
    assert_eq!(
        hex_encode(&hmac_sha1(&[0x0b; 20], b"Hi There")),
        "b617318655057264e28bc0b6fb378c8ef146be00"
    );
    // §3, test case 2 — a key that is a word, and a longer message.
    assert_eq!(
        hex_encode(&hmac_sha1(b"Jefe", b"what do ya want for nothing?")),
        "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
    );
    // §3, test case 6 — a key longer than the 64-byte block, so the key-hashing branch runs.
    assert_eq!(
        hex_encode(&hmac_sha1(
            &[0xaa; 80],
            b"Test Using Larger Than Block-Size Key - Hash Key First"
        )),
        "aa4ae5e15272d00e95705637ce8a3b55ed402112"
    );
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
            now: signed_at(&row, &spec) + window - 1,
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
            now: signed_at(&row, &spec) + window + 1,
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
            now: signed_at(&row, &spec) - window - 1,
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

fn signed_at(row: &Row, spec: &HmacSpec) -> i64 {
    let (_, value) = row.timestamp.expect("a timestamped row");
    parse_timestamp(value, spec.timestamp_format.unwrap_or_default())
        .expect("the fixture's timestamp parses in the spelling its scheme declares")
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

/// A window a host cannot parse is a window it will not apply — and the loader now says so.
///
/// This used to be a per-provider gate, because the loader required a `tolerance` on a timestamped
/// scheme and had no opinion about its shape: `tolerance = "banana"` loaded, and the real window
/// became whatever each host decided at runtime. C-141 moved the opinion to the loader, so the gate
/// is now an invariant of *loading* rather than a sweep over what happens to be shipped — every spec
/// [`shipped_specs`] returns came through [`provider::load`] and therefore already parses.
///
/// What is left to check is the loader's refusal itself, which is the thing the sweep was standing in
/// for. Both halves matter: a spelling nobody can read, and a window so long it is one only in name.
#[test]
fn a_tolerance_no_host_could_apply_does_not_load() {
    let with_tolerance = |value: &str| {
        fixture(&format!(
            r#"
[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Acme-Signature"
signed = "{{timestamp}}.{{body}}"
timestamp = {{ source = "header", name = "X-Acme-Timestamp" }}
secret = "acme.webhook_secret"
tolerance = "{value}"
"#
        ))
    };

    assert!(
        provider::load("providers/fixture.toml", &with_tolerance("5m")).is_ok(),
        "a window the vendor documents must still load"
    );

    // The last is the overflow case: `*` panicked here in a debug build and, in a release build,
    // wrapped `i64::MAX * 60` to a negative window that loaded cleanly.
    for unusable in ["banana", "5", "0s", "2h", "7d", "9223372036854775807m"] {
        let error = provider::load("providers/fixture.toml", &with_tolerance(unusable))
            .err()
            .unwrap_or_else(|| {
                panic!(
                    "`tolerance = {unusable:?}` must be refused at load: the window is the only \
                     bound on how long a captured signature stays usable, so one no host can apply \
                     is a replay window in name only"
                )
            });
        assert!(
            format!("{error}").contains("tolerance"),
            "the refusal must name the field it rejected:\n{error}"
        );
    }

    // And the sweep the loader replaced, kept as the statement that it is now redundant.
    for (provider_id, channel, spec) in shipped_specs() {
        if let Some(tolerance) = spec.tolerance.as_deref() {
            assert!(
                parse_tolerance(tolerance).is_ok(),
                "providers/{provider_id}.toml's `{channel}` binding loaded with \
                 `tolerance = {tolerance:?}`, which means the loader stopped parsing it"
            );
        }
    }
}

/// The timestamp *format* axis, and why it is not cosmetic.
///
/// `HmacSpec` used to say where the timestamp was read from and never how it was spelled, so the
/// reference verifier sniffed. Sniffing looks harmless until a spelling is ambiguous:
/// `20220505183228` reads as a date to a person and as unix seconds to `parse`, and a sniffing host
/// would compute a window against a moment 600,000 years away — accepting a replay of any age. With
/// the axis declared, a value in the wrong spelling is **refused** rather than reinterpreted.
#[test]
fn the_declared_timestamp_format_is_read_instead_of_sniffed() {
    let zendesk = matrix()
        .into_iter()
        .find(|row| row.vendor == "zendesk")
        .expect("the matrix carries the RFC 3339 row");
    let spec = zendesk.spec();
    assert_eq!(
        spec.timestamp_format,
        Some(TimestampFormat::Rfc3339),
        "the row declares its spelling rather than leaving it to be guessed"
    );

    // Each vendor's own spelling parses under its own declared format…
    assert_eq!(
        parse_timestamp("1531420618", TimestampFormat::UnixSeconds),
        Ok(1_531_420_618)
    );
    assert_eq!(
        parse_timestamp("2022-05-05T18:32:28Z", TimestampFormat::Rfc3339),
        Ok(1_651_775_548)
    );

    // …and neither is silently read under the other's.
    assert!(parse_timestamp("2022-05-05T18:32:28Z", TimestampFormat::UnixSeconds).is_err());
    assert!(parse_timestamp("1531420618", TimestampFormat::Rfc3339).is_err());

    // The ambiguous spelling: a compact date that is also a valid integer. Declared as RFC 3339 it
    // is refused; a sniffing host accepted it as a timestamp 600,000 years in the future, against
    // which no five-minute window can be stale.
    assert!(parse_timestamp("20220505183228", TimestampFormat::Rfc3339).is_err());
    assert_eq!(
        parse_timestamp("20220505183228", TimestampFormat::UnixSeconds),
        Ok(20_220_505_183_228),
        "the sniffing hazard, stated: it is a perfectly good integer"
    );

    // A format declared where nothing reads a timestamp is refused, on the same ground as an unused
    // selector: it describes the spelling of a value nothing reads.
    let unused = fixture(
        r#"
[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Acme-Signature"
signed = "{body}"
timestamp_format = "rfc3339"
secret = "acme.webhook_secret"
"#,
    );
    let error = provider::load("providers/fixture.toml", &unused)
        .expect_err("a format with no timestamp to spell must be refused");
    assert!(
        format!("{error}").contains("timestamp_format"),
        "the refusal must name the field it rejected:\n{error}"
    );
}

/// A verification timestamp read from the body inverts the order verification depends on.
///
/// `HmacSpec::timestamp` is a full [`Selector`], so `source = "body"` is spellable — and incoherent:
/// finding the value would mean parsing the bytes whose trustworthiness that value helps decide,
/// which puts a parser in front of an anonymous caller. The reference verifier already refuses it at
/// request time (`Refusal::CannotVerify`), and flux refuses it in its own request path, but a
/// connector could still *ship* it. Now it cannot load.
#[test]
fn a_body_sourced_verification_timestamp_does_not_load() {
    let from_body = fixture(
        r#"
[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Acme-Signature"
signed = "{timestamp}.{body}"
timestamp = { source = "body", name = "event.created_at" }
secret = "acme.webhook_secret"
tolerance = "5m"
"#,
    );
    let error = provider::load("providers/fixture.toml", &from_body)
        .expect_err("a body-sourced verification timestamp must be refused at load");
    let error = format!("{error}");
    assert!(
        error.contains("body") && error.contains("verified"),
        "the refusal must say why the ordering is the problem:\n{error}"
    );
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
