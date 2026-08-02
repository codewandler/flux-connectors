//! The reverse call direction: the events a vendor sends *us*, and the channel bindings that make
//! one reachable from flux.
//!
//! Outbound is an [`Operation`](crate::Operation): flux calls the vendor. Inbound is an
//! [`EventDecl`]: the vendor calls flux. A [`ChannelBinding`] is neither — it is the **composition**
//! of the two, and that is the whole idea of this module.
//!
//! # Why a binding is a composition and not a primitive
//!
//! flux hard-codes its ingress surfaces: `flux-channels`' adapter dispatch is a closed `match` on a
//! `kind` string, and one of its arms is 218 lines of Slack-specific Rust. That adapter's last act is
//! to build a `chat.postMessage` request by hand — which is `slack-chat-post-message`, an operation
//! this repository already compiles from `providers/slack.toml`. flux is hand-writing an outbound
//! call the connector already generates.
//!
//! So a binding needs no new primitive. It names:
//!
//! - a **transport** flux owns ([`Transport`]) — webhook, socket, or poll;
//! - the **events** it carries, which are [`EventDecl`]s of the same service;
//! - a **payload map**, turning the vendor's envelope into the symbols a flow reads;
//! - and a **reply** ([`Reply`]) — an *operation of this same connector*, with its parameters bound
//!   from that payload map.
//!
//! Both halves already exist in the IR. Nothing new is emitted into the module: the reply op is
//! already there, and the binding itself reaches only the manifest and the catalogue.
//!
//! # What is deliberately absent
//!
//! **No URL, no secret, no schedule.** The endpoint address is the operator's deployment detail, the
//! secret is a credential *name* the host resolves (principle 4), and the loop that drives a
//! [`Transport::Poll`] binding is an operator's `channel schedule` + `trigger` — a documented program
//! pattern, not something this repository emits or runs. flux-connectors ships no runtime.
//!
//! # Polling is lossy, so the cursor carries the correctness
//!
//! flux's cron is one in-process task per channel, and *missed-tick replay is a named non-goal* of
//! its design. A restart drops ticks. That is why [`ChannelBinding::cursor`] is **required** for
//! [`Transport::Poll`] rather than merely permitted: the schedule cannot be trusted to have run, so
//! the only thing that makes a poll correct is resuming from a recorded position. A poll binding
//! without a cursor is a silent gap, and the loader refuses it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    ir::{default_service, is_default_service, JsonSchema},
    AuthRequirement,
};

/// The declarative RFC 6455 handshake for a [`Transport::Socket`] binding.
///
/// Every value is inert data. A host may turn it into a prepared plan, but this crate never resolves
/// a host, reads a credential or opens a socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocketConnectSpec {
    /// A path relative to the binding service's `base_url`.
    pub path: String,
    /// Query parameter name → fixed value or `{configuration_field}` template.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, String>,
    /// Fixed, non-secret request headers. Handshake-owned and authentication headers are refused.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// Authentication alternatives, with the same AND/OR meaning as operation authentication.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth: Vec<AuthRequirement>,
    /// WebSocket subprotocols, in preference order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subprotocols: Vec<String>,
}

/// How an inbound event reaches flux.
///
/// The three exhaust what vendors actually offer, and the split is what makes "inbound" an
/// abstraction over transports rather than a synonym for "webhook".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// The vendor POSTs to an endpoint the operator exposes. The overwhelming majority, and the only
    /// one that needs [`VerificationScheme`] — an open endpoint must prove who called it.
    Webhook,
    /// *We* connect outward and hold a stream the vendor pushes down (Slack Socket Mode). Inbound in
    /// data direction, outbound in operation — which is why it needs no signature: the connection is
    /// already authenticated by the credential that opened it.
    Socket,
    /// The vendor offers no push at all, so a cursor operation is called on a schedule. See the
    /// module docs on why [`ChannelBinding::cursor`] is mandatory here.
    Poll,
}

/// Where on an inbound request a single named value is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSource {
    /// An HTTP header of the inbound request.
    Header,
    /// A field of the decoded body, addressed by the dotted path grammar — see [`validate_path`].
    Body,
}

/// The digest a [`HmacSpec`] signs with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Digest {
    /// SHA-1. Legacy — GitHub's `X-Hub-Signature`, superseded by its SHA-256 header.
    Sha1,
    /// SHA-256. What every scheme worth adopting uses.
    Sha256,
}

/// How a [`HmacSpec`]'s digest is spelled in the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    /// Lowercase hex — GitHub, Stripe, Slack.
    Hex,
    /// Base64 — Zendesk.
    Base64,
}

/// One named value read off an inbound request: the event discriminator, or the delivery id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    /// Where to read it from.
    pub source: FieldSource,
    /// The header name, or the dotted body path.
    pub name: String,
}

/// How the value [`HmacSpec::timestamp`] selects is **spelled**.
///
/// A separate axis from the selector, because the selector says only *where* the timestamp is read
/// from. Slack and Stripe send unix seconds; Zendesk sends RFC 3339. Without this, a host has to sniff
/// — try an integer, fall back to a date shape — and sniffing is exactly the guessing the selector was
/// added to stop. It is also not free: `20220505183228` is a plausible timestamp spelling that parses
/// as an integer and lands 600,000 years from now, and a sniffing host would apply the window to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    /// Whole seconds since the unix epoch, as a decimal integer — `1531420618`. Slack, Stripe,
    /// GitHub, and the default, because it is what the large majority of vendors send.
    #[default]
    UnixSeconds,
    /// RFC 3339, as `2022-05-05T18:32:28Z` — Zendesk.
    Rfc3339,
}

/// The parameters of one HMAC webhook signature scheme.
///
/// **This struct is the finding the inbound design rests on.** GitHub, Stripe, Slack and Zendesk each
/// document a bespoke-looking signature, and they vary along exactly four axes: which digest, how it
/// is encoded, what string is signed, and how long a signature stays acceptable. Four vendors, four
/// "unique" schemes, one parameterized algorithm — which is a struct, not a script, and therefore
/// something a compiler can carry.
///
/// | vendor | header | algorithm | encoding | signed | window |
/// |---|---|---|---|---|---|
/// | GitHub | `X-Hub-Signature-256` | sha256 | hex, `sha256=` | `{body}` | — |
/// | Stripe | `Stripe-Signature` | sha256 | hex | `{timestamp}.{body}` | tolerance |
/// | Slack | `X-Slack-Signature` | sha256 | hex, `v0=` | `v0:{timestamp}:{body}` | 5m |
/// | Zendesk | `X-Zendesk-Webhook-Signature` | sha256 | base64 | `{timestamp}{body}` | tolerance |
/// | Twilio | `X-Twilio-Signature` | sha1 | base64 | `{url}{sorted_form}` | — |
///
/// Twilio is the row that cost this struct a widened vocabulary rather than a new field (C-188). It
/// signs neither the raw bytes nor any constant: the request URL, then the POST parameters decoded,
/// sorted by name and re-joined. Its shape was the argument for the axes being right — every other
/// axis fits unchanged — and against `signed` being a template over two names.
///
/// **Nothing here verifies anything.** The comparison runs in flux, over the raw request bytes,
/// before any parsing — verifying a re-serialized body fails on byte-identical-but-reordered JSON,
/// and any "normalize then verify" step is a bypass. This type only *declares* the parameters that
/// comparison uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmacSpec {
    /// The digest to compute.
    pub algorithm: Digest,
    /// How the digest is spelled in the header.
    pub encoding: Encoding,
    /// The header carrying the signature, e.g. `X-Hub-Signature-256`.
    pub header: String,
    /// A literal prefix the header value carries before the digest, e.g. `sha256=` or `v0=`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// The string that is signed, as a template over the closed vocabulary
    /// [`SIGNED_PLACEHOLDERS`] names.
    ///
    /// - **`{body}`** — the raw request bytes, spliced in exactly as they arrived.
    /// - **`{sorted_form}`** — the body read as `application/x-www-form-urlencoded`, each field
    ///   percent-decoded, the fields sorted by name, and every name concatenated straight onto its
    ///   value with **no delimiter** anywhere. Twilio's scheme, and the one placeholder that is a
    ///   *derivation* rather than a splice.
    /// - **`{timestamp}`** — the value [`timestamp`](Self::timestamp) selects, spelled as
    ///   [`timestamp_format`](Self::timestamp_format) says.
    /// - **`{url}`** — the full request URL the vendor was configured to call, query string
    ///   included. Supplied by the transport at request time; **never** carried in this repository,
    ///   which ships no endpoint address for the reason the module docs state.
    ///
    /// Any other placeholder is a loader error — a template the host cannot fill would fail open or
    /// fail confusingly, and neither is acceptable on an authentication path.
    ///
    /// **The template must interpolate one of [`PAYLOAD_PLACEHOLDERS`], and that is the
    /// load-bearing rule of this struct.** A template that covers no payload signs a string the
    /// request never enters, so one captured signature verifies every forged payload — for the whole
    /// [`tolerance`](Self::tolerance) window, or forever without one. `signed = "{timestamp}"` and
    /// `signed = "{url}"` are both perfectly well-formed templates that do exactly that, which is
    /// why the rule is stated here rather than left to the shape of the template: the defect needs no
    /// typo, and everything else about such a declaration reads as correct.
    ///
    /// # Two things `{sorted_form}` is not
    ///
    /// It is **not a nesting convention.** `BodyEncoding::Form` refuses a nested *outbound* body
    /// because vendors disagree about how to spell one (`metadata[key]`, `a[b]`, `a[b][]`), and the
    /// form encoder that would settle it is upstream flux work. None of that reaches here: a form
    /// body on the wire is already a flat sequence of `name=value` pairs, and this derivation reads
    /// that sequence rather than producing one. The two gaps run in opposite directions and do not
    /// touch.
    ///
    /// It is **not a licence to sort, join or transform in the template.** The template still only
    /// concatenates literal text around named values. What sorts is one function in the host, the
    /// same one for every vendor that names the placeholder — which is the difference between
    /// declaring a derivation and shipping a per-vendor script.
    pub signed: String,
    /// Where the `{timestamp}` in [`signed`](Self::signed) is read from — Slack's
    /// `X-Slack-Request-Timestamp`.
    ///
    /// Required exactly when `signed` interpolates `{timestamp}`, and refused when it does not. The
    /// template alone cannot say *where* the value comes from, and a host left to guess would either
    /// fail every request or, worse, fall back to its own clock — which verifies nothing.
    ///
    /// **[`FieldSource::Body`] is refused.** It is spellable in the type and incoherent in practice: a
    /// timestamp read from the body has to be parsed *before* the bytes carrying it are verified, which
    /// inverts the order that makes verification mean anything and exposes a parser to any anonymous
    /// caller. flux refuses it in its own request path (C-291); the loader refuses it first, so the
    /// failure lands in a build rather than in an operator's runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Selector>,
    /// How the selected timestamp is spelled. Absent means [`TimestampFormat::UnixSeconds`].
    ///
    /// Refused when `signed` does not interpolate `{timestamp}`, on the same ground as an unused
    /// selector: it would describe the spelling of a value nothing reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_format: Option<TimestampFormat>,
    /// The **name** of the credential holding the shared secret — never a value.
    ///
    /// Resolves to an [`AuthMethod`](crate::AuthMethod) of this connector, declared with
    /// [`AuthScheme::Signing`](crate::AuthScheme::Signing). One credential namespace, so the
    /// manifest's credential list stays complete and the host has one place to look.
    pub secret: String,
    /// How old a signed request may be, as a duration (`5m`, `300s`) — the grammar
    /// [`parse_tolerance`] defines, and the loader parses.
    ///
    /// Mandatory when `signed` interpolates `{timestamp}`: a timestamped scheme without a window is a
    /// signature that replays forever, which is strictly worse than not timestamping at all because
    /// it reads as though replay were handled. Requiring one is not enough on its own, though —
    /// `tolerance = "banana"` is a declared window nobody can apply, and it reads as though replay
    /// were handled just as convincingly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<String>,
}

/// How an inbound request proves it came from the vendor.
///
/// Externally tagged, exactly as [`AuthScheme`](crate::AuthScheme) spells its parameterized variants
/// — `verification = "none"`, or a `[channels.verification.hmac]` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum VerificationScheme {
    /// This transport cannot be verified, stated deliberately.
    ///
    /// Not the same as saying nothing. See [`ChannelBinding::verification`] for the tri-state: an
    /// unset verification on a webhook is a loader error, and this variant is how an author says
    /// "the vendor publishes no signature" in a way the manifest can then say loudly.
    None,
    /// An HMAC over the raw request bytes.
    Hmac(HmacSpec),
}

/// One event a vendor sends: its name, what narrows it, and what it looks like.
///
/// An event keeps its **vendor name**, exactly as an operation keeps its vendor operation name. A
/// normalized cross-vendor event taxonomy is a different product, and one that discards the fidelity
/// this repository exists to preserve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventDecl {
    /// The event name, e.g. `issues.opened`. Unique across **all three member kinds** of its service,
    /// and the label a flux `trigger { on = … }` matches.
    pub name: String,
    /// The exact value carried by the binding discriminator when it differs from [`name`](Self::name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_value: Option<String>,
    /// The [`Service`](crate::Service) this event belongs to — exactly one, always a concrete name,
    /// with the same reasoning [`Operation::service`](crate::Operation::service) records.
    #[serde(
        default = "default_service",
        skip_serializing_if = "is_default_service"
    )]
    pub service: String,
    /// What the event means, in one line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Whether a product offers this event **on** by default when a user connects.
    ///
    /// Defaults to `true`. Slack's `message` is the case for saying otherwise: it fires for every
    /// human message in every channel the app is in, so defaulting it on turns a connection into a
    /// firehose nobody asked for. Today that warning exists only in prose inside `description`.
    #[serde(default = "default_selected", skip_serializing_if = "is_true")]
    pub default: bool,
    /// An optional grouping label, so a long event list renders as sections rather than forty
    /// checkboxes.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    /// Field equalities that narrow a coarse vendor event into this one.
    ///
    /// GitHub sends one `issues` event with an `action` field; `{ action = "opened" }` is what makes
    /// `issues.opened` a distinct thing a trigger can match. Empty means the discriminator alone
    /// identifies it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub when: BTreeMap<String, JsonSchema>,
    /// The JSON Schema of the event payload, when the vendor publishes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<JsonSchema>,
}

/// The outbound half of a [`ChannelBinding`]: which operation answers, and how its parameters are
/// filled from the inbound payload.
///
/// This is the part that removes hand-written Rust from flux. `flux-channels`' Slack adapter ends by
/// constructing a `chat.postMessage` with `channel` and `thread_ts` taken off the message it just
/// received; expressed here, that is an operation id and a two-entry map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    /// The [`Operation::id`](crate::Operation::id) that answers. It must belong to this connector —
    /// the loader refuses a dangling reference, because a binding naming an operation nobody emits is
    /// a channel that cannot reply.
    pub operation: String,
    /// The reply parameter that carries the **journey's own output**.
    ///
    /// The rest of a reply is filled from the inbound payload, but its most important field is not:
    /// what a flow computed has no dotted path into the event that triggered it. flux's Slack adapter
    /// makes the same distinction in code — it joins the `JourneyRun` results into one string and
    /// passes that as `text`, while `channel` and `thread_ts` come off the message it received.
    /// `result = "text"` is that line, declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Reply parameter name → [`ChannelBinding::payload`] key.
    ///
    /// Every **required** parameter of the reply operation must be covered, by this map or by
    /// [`result`](Self::result). A partially bound reply would compile and then fail at the first
    /// delivery, which is exactly the plausible-but-wrong artifact this repository refuses to emit.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bind: BTreeMap<String, String>,
}

/// How a webhook gets registered with the vendor, when the vendor has an API for it.
///
/// Registering a webhook is an **ordinary outbound write** — an authorized, approvable operation like
/// any other, never a build-time side effect. What was missing was the link: a binding knew its
/// reply and its cursor but had no way to say which operation subscribes it, so a product could not
/// offer a "Connect" button that did anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subscription {
    /// The operation that registers the endpoint.
    pub subscribe: String,
    /// The operation that removes it. Absent when the vendor offers no removal — which is worth
    /// knowing, because it means a disconnect leaves the vendor sending to a dead URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsubscribe: Option<String>,
    /// The operation that lists existing registrations, for reconciling duplicates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    /// Which parameter of [`subscribe`](Self::subscribe) receives **our** public callback URL.
    ///
    /// The URL itself is never here: it is the product's deployment detail, and a connector that
    /// carried one would be describing someone else's infrastructure. This says only where to put it.
    pub callback_param: String,
}

/// What a human must do in the vendor's own dashboard, for vendors with no subscription API.
///
/// Slack is the case: there is no Web API method that registers an Events API endpoint, so somebody
/// opens `api.slack.com/apps` and pastes a URL. That work does not disappear because it is
/// undeclared — it just moves into a support article nobody keeps current.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualSetup {
    /// The vendor's own page for this, so a UI can link out rather than restate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    /// The steps, in order, each one thing a person does. Rendered as a numbered list beside the
    /// callback URL the product displays.
    pub steps: Vec<String>,
}

/// One ingress surface a connector describes, over a transport flux owns.
///
/// See the module docs for why this is a composition rather than a primitive, and
/// `docs/designs/inbound-events.md` for the five concerns every vendor webhook decomposes into.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelBinding {
    /// The binding name, e.g. `slack`. Unique across **all three member kinds** of its service.
    pub name: String,
    /// The [`Service`](crate::Service) this binding belongs to — exactly one, always concrete.
    #[serde(
        default = "default_service",
        skip_serializing_if = "is_default_service"
    )]
    pub service: String,
    /// What the binding is for, in one line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// How events reach flux.
    pub transport: Transport,
    /// The generic RFC 6455 handshake, valid only for [`Transport::Socket`].
    ///
    /// Socket bindings without this block are vendor-specific transports such as Slack Socket Mode
    /// and remain the consuming runtime's explicit responsibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect: Option<SocketConnectSpec>,
    /// The [`EventDecl`] names this binding carries, all from the same service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    /// How an inbound request proves it came from the vendor. **Tri-state, deliberately** — the same
    /// shape and the same reasoning as [`Operation::auth`](crate::Operation::auth):
    ///
    /// - `None` — **unset**. Legal for [`Transport::Socket`] and [`Transport::Poll`], which
    ///   authenticate by the credential that opens the connection. A loader error for
    ///   [`Transport::Webhook`], because silence on an open endpoint is how an unverified event gets
    ///   presented as a trusted one.
    /// - `Some(VerificationScheme::None)` — **explicitly unverifiable**. The author states that the
    ///   vendor publishes no signature; the manifest then says so loudly rather than omitting it.
    /// - `Some(VerificationScheme::Hmac(..))` — verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<VerificationScheme>,
    /// Where to read *which event this is*, when the transport carries it out of band — GitHub's
    /// `X-GitHub-Event` header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Selector>,
    /// Where to read the vendor's redelivery id, so a flow can dedupe at-least-once delivery.
    ///
    /// Vendors redeliver. Without this a retried webhook is indistinguishable from a second real
    /// event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_id: Option<Selector>,
    /// Flow symbol → dotted source path into the inbound envelope.
    ///
    /// The keys are what a journey reads (`$text`, `{user}`); the values address the vendor's
    /// envelope with the **same dotted grammar** [`Param::wire`](crate::Param::wire) already uses for
    /// request bodies. One path grammar in the repository, not two.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub payload: BTreeMap<String, String>,
    /// Deliver the complete decoded JSON event rather than projecting fields from it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub payload_root: bool,
    /// The operation that answers an event on this binding, if any.
    ///
    /// Absent for a fire-and-forget binding — a poll, or a webhook whose vendor expects only a 200.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<Reply>,
    /// The cursor operation a [`Transport::Poll`] binding calls. **Required** for that transport and
    /// refused for the others — see the module docs on why the cursor, not the schedule, is what
    /// makes polling correct.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// How this binding gets registered with the vendor, when the vendor has an API for it.
    ///
    /// A [`Transport::Webhook`] binding declares this **or** [`setup`](Self::setup): a public URL
    /// with no instructions for what to do with it is not a configurable surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<Subscription>,
    /// What a human does in the vendor's dashboard, when there is no subscription API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<ManualSetup>,
    /// The **suggested** poll interval (`5m`), for [`Transport::Poll`] only.
    ///
    /// Advisory, and it is worth being precise about why: the operator writes the actual schedule in
    /// their own program, this repository runs nothing, and flux's cron drops ticks across a restart.
    /// So this is a hint for documentation and a starting value, never a guarantee about cadence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
}

/// Whether `path` is a well-formed dotted path into an inbound envelope.
///
/// The grammar is [`Param::wire`](crate::Param::wire)'s: dot-separated segments, as in
/// `event.thread_ts`. Deliberately not JSONPath — a second path language in the same repository would
/// be a homegrown DSL the north star forbids, and the existing one already addresses nested JSON.
pub fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("a source path must not be empty".to_owned());
    }
    if path.starts_with('.') || path.ends_with('.') || path.contains("..") {
        return Err(format!(
            "{path:?} has an empty segment; a source path reads as `event.thread_ts`, never with a \
             leading, trailing or doubled `.`"
        ));
    }
    if let Some(bad) = path.chars().find(|c| c.is_whitespace()) {
        return Err(format!("{path:?} contains whitespace ({bad:?})"));
    }
    Ok(())
}

/// Whether `symbol` can be bound as a Flux flow symbol.
///
/// A [`ChannelBinding::payload`] key becomes a symbol a journey reads, so it has to be spellable as
/// one. Snake case rather than flux's full `decl_name` grammar, which also admits `-`: `$a-b` reads
/// as a subtraction, so admitting a hyphen here would produce a binding that parses as something
/// else entirely.
pub fn validate_symbol(symbol: &str) -> Result<(), String> {
    if symbol.is_empty() {
        return Err("a payload symbol must not be empty".to_owned());
    }
    let mut chars = symbol.chars();
    let first = chars.next().unwrap_or_default();
    if !first.is_ascii_lowercase() {
        return Err(format!(
            "payload symbol {symbol:?} must start with a lowercase ASCII letter — it is bound as a \
             Flux symbol"
        ));
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(format!(
            "payload symbol {symbol:?} is not snake case; a Flux symbol admits lowercase ASCII \
             letters, digits and `_` (never `-`, which reads as subtraction)"
        ));
    }
    Ok(())
}

/// `serde(default)` for [`EventDecl::default`]: an event is offered on unless it says otherwise.
fn default_selected() -> bool {
    true
}

/// `skip_serializing_if` for [`EventDecl::default`], so the common case adds nothing to the encoding.
fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The placeholders [`HmacSpec::signed`] may interpolate.
///
/// # The decision this list records: a widened vocabulary, not a named set of vendor schemes
///
/// C-188 had two defensible shapes to choose between, and the choice is recorded here because the
/// list is where a future author meets it. The alternative was a closed set of *named* schemes —
/// `verification.twilio_v1`, `verification.stripe_v1` — with the derivation living in the verifier
/// under that name.
///
/// The template won, on three grounds:
///
/// 1. **A closed set of named schemes is a `match vendor`, spelled as data.**
///    `tests/verification_conformance.rs`'s first rule is that the reference verifier contains no
///    per-vendor branch, "and there cannot be one: a per-vendor branch here would prove that four
///    schemes can be verified by four implementations, which nobody doubted." `twilio_v1` is that
///    branch. It also moves the vendor's *parameters* — the digest, the encoding, the header — out
///    of the connector and into the verifier, where a drift check cannot see them.
/// 2. **This list is already the closed set.** The expressiveness objection to a template is that an
///    author can write a derivation the verifier does not compute — and the loader refuses exactly
///    that, by name, at load. The template has no operators, no repetition and no ordering
///    primitive; the only thing it composes is literal text, which is where the vendors genuinely
///    differ (Slack's `v0:`, Stripe's `.`, Twilio's nothing-at-all).
/// 3. **A named set refuses the unknown vendor**, which sounds like the safe default and is not.
///    Refusing at load is only valuable when the alternative was a *wrong* verification; a vendor
///    whose axes are already in this vocabulary would be refused for having no name, and the honest
///    workaround — declare no binding — is the unverified endpoint the story is closing.
///
/// What keeps (1)–(3) from being a licence to keep adding names is [`PAYLOAD_PLACEHOLDERS`]: a
/// placeholder may be added only if the loader can still tell whether the payload enters the signed
/// string. That is the rule the whole struct rests on, and it is stated as a property of the
/// vocabulary rather than of any one name.
pub const SIGNED_PLACEHOLDERS: [&str; 4] = ["body", "sorted_form", "timestamp", "url"];

/// The placeholders through which the **request payload** enters the signed string.
///
/// [`HmacSpec::signed`] must interpolate at least one of these, and this list — rather than the word
/// `{body}` — is the load-bearing rule of the struct. The distinction became real the moment the
/// vocabulary widened: `{url}` is a *per-endpoint constant*, so `signed = "{url}"` signs the same
/// string for every delivery that endpoint will ever receive. That is C-141's `signed = "{timestamp}"`
/// with a longer constant and, since a URL-signing vendor carries no timestamp, no
/// [`tolerance`](HmacSpec::tolerance) to bound the replay either. It would have loaded the moment
/// `{url}` became fillable, under a rule that read as though it still said what it used to say.
///
/// `{sorted_form}` is here and `{url}` is not because the test is not "does the placeholder vary" but
/// "does the *payload* reach the digest". A reassembled form is the payload, rearranged; a URL is the
/// address it was sent to.
pub const PAYLOAD_PLACEHOLDERS: [&str; 2] = ["body", "sorted_form"];

/// The placeholder names `signed` actually interpolates, in order of appearance.
///
/// Used by the loader to refuse a template the host could not fill, and to require a
/// [`HmacSpec::tolerance`] exactly when `{timestamp}` is present.
///
/// # An unterminated `{` is reported, never swallowed
///
/// `"v0:{timestamp}:{body"` — one missing brace, a plausible typo in a provider file — is not a
/// template with one placeholder. Reporting it as one is how a signature comes to authenticate
/// nothing: every check the loader makes passes (the placeholder list is non-empty, every name is
/// fillable, the timestamp selector and the tolerance are present), and the host then signs
/// `v0:1531420618:{body` — a string that does not contain the body at all. A signature captured
/// from one delivery verifies **any** forged payload for as long as the window lasts.
///
/// So the unterminated fragment comes back verbatim, opening brace included, as a placeholder name
/// no host can fill. It is not in [`SIGNED_PLACEHOLDERS`], so the loader's existing refusal catches
/// it and names the template in the error.
pub fn signed_placeholders(signed: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = signed;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            found.push(format!("{{{after}"));
            break;
        };
        found.push(after[..close].to_owned());
        rest = &after[close + 1..];
    }
    found
}

/// The longest replay window a webhook scheme may declare, in seconds.
///
/// One hour, and the bound is deliberately opinionated rather than merely arithmetic. Every vendor in
/// the matrix documents minutes — Slack five, Stripe five by default — because the window is how long
/// a captured request stays usable, and there is no delivery-latency argument for an hour. A
/// `tolerance = "7d"` parses, satisfies every other rule, and is a replay window in name only.
pub const MAX_TOLERANCE_SECONDS: i64 = 3600;

/// A [`HmacSpec::tolerance`] as a whole number of seconds: `5m`, `300s`, `1h`.
///
/// # Why the loader parses this rather than the host
///
/// The window is the *only* bound on how long a captured signature stays usable, so a spelling no host
/// can read is a replay window that does not exist — and the declaration still reads as though replay
/// had been handled. `tolerance = "banana"` used to load: the loader required a window on any
/// timestamped scheme but had no opinion about its shape, which left the actual window to whatever each
/// host decided at runtime (reject everything, or apply no window at all). Parsing here makes it one
/// number, decided once, in a build.
///
/// The grammar is deliberately tiny: a whole number and one of `s`, `m`, `h`. No fractions, no
/// compound `1m30s`, no bare integer — a bare `300` would have to guess a unit, and this repository
/// refuses to be the layer that guesses.
pub fn parse_tolerance(tolerance: &str) -> Result<i64, String> {
    let (digits, unit, scale) = match tolerance.strip_suffix('s') {
        Some(digits) => (digits, "s", 1),
        None => match tolerance.strip_suffix('m') {
            Some(digits) => (digits, "m", 60),
            None => match tolerance.strip_suffix('h') {
                Some(digits) => (digits, "h", 3600),
                None => {
                    return Err(format!(
                        "{tolerance:?} names no unit; a window reads as `5m`, `300s` or `1h`"
                    ))
                }
            },
        },
    };

    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "{tolerance:?} is not a whole number of {unit:?} units; a window reads as `5m`, `300s` \
             or `1h`"
        ));
    }
    // `checked_mul`, not `*`. Scaling a count into seconds is fallible arithmetic on author input, and
    // both of its failure modes are the very defect this function exists to close: in a debug build
    // `"9223372036854775807m"` panics inside the loader, and in a release build it *wraps* — to `-60`,
    // a negative window that passes every check below and ships a declared replay bound no host could
    // apply. The contract is already `Result`, so the failure has somewhere to go. One message for
    // both modes, because the author's mistake is the same either way: the number is too big.
    let too_large = || format!("{tolerance:?} is too large to be a window");
    let seconds = digits
        .parse::<i64>()
        .ok()
        .and_then(|count| count.checked_mul(scale))
        .ok_or_else(too_large)?;

    // A zero window accepts only a request signed in the same second the host checks it, which is not
    // a strict policy but a broken one: it rejects nearly every genuine delivery.
    if seconds == 0 {
        return Err(format!(
            "{tolerance:?} is a window of no length, which rejects every delivery that took any time \
             to arrive; state the window the vendor documents"
        ));
    }
    if seconds > MAX_TOLERANCE_SECONDS {
        return Err(format!(
            "{tolerance:?} lets a captured signature be replayed for {seconds}s; a webhook window is \
             minutes, not hours (at most {MAX_TOLERANCE_SECONDS}s)"
        ));
    }
    Ok(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_reject_empty_segments_and_whitespace() {
        assert!(validate_path("event.thread_ts").is_ok());
        assert!(validate_path("text").is_ok());
        assert!(validate_path("").is_err());
        assert!(validate_path(".event").is_err());
        assert!(validate_path("event.").is_err());
        assert!(validate_path("event..text").is_err());
        assert!(validate_path("event .text").is_err());
    }

    #[test]
    fn symbols_are_snake_case_because_a_hyphen_reads_as_subtraction() {
        assert!(validate_symbol("thread_ts").is_ok());
        assert!(validate_symbol("text").is_ok());
        assert!(validate_symbol("a1").is_ok());
        assert!(validate_symbol("").is_err());
        assert!(validate_symbol("Thread").is_err());
        assert!(validate_symbol("1text").is_err());
        assert!(validate_symbol("thread-ts").is_err());
    }

    #[test]
    fn signed_templates_report_their_placeholders_in_order() {
        assert_eq!(signed_placeholders("{body}"), vec!["body"]);
        assert_eq!(
            signed_placeholders("v0:{timestamp}:{body}"),
            vec!["timestamp", "body"]
        );
        assert_eq!(
            signed_placeholders("{url}{sorted_form}"),
            vec!["url", "sorted_form"],
            "Twilio's scheme: two placeholders and not one character of literal text between them"
        );
        assert!(signed_placeholders("nothing here").is_empty());
    }

    /// Every way of covering the payload is a placeholder the host can fill, and the two lists must
    /// not drift apart.
    ///
    /// A name in [`PAYLOAD_PLACEHOLDERS`] but not in [`SIGNED_PLACEHOLDERS`] is the worst outcome
    /// available here: the loader would accept it as covering the payload while the template check
    /// refused it as unfillable, so the two rules would contradict each other on the same
    /// declaration. Cheap to assert, and not otherwise checked anywhere.
    #[test]
    fn every_payload_placeholder_is_one_the_host_can_fill() {
        for name in PAYLOAD_PLACEHOLDERS {
            assert!(
                SIGNED_PLACEHOLDERS.contains(&name),
                "{{{name}}} covers the payload but is not fillable"
            );
        }
        assert!(
            !PAYLOAD_PLACEHOLDERS.contains(&"url"),
            "the request URL is the same for every delivery to one endpoint, so a signature over it \
             alone proves nothing about the payload — see PAYLOAD_PLACEHOLDERS' own docs"
        );
        assert!(
            !PAYLOAD_PLACEHOLDERS.contains(&"timestamp"),
            "C-141's hole: a timestamped signature over no payload verifies every forgery in the \
             window"
        );
    }

    #[test]
    fn a_window_is_a_whole_number_of_seconds_minutes_or_hours() {
        assert_eq!(parse_tolerance("300s"), Ok(300));
        assert_eq!(parse_tolerance("5m"), Ok(300));
        assert_eq!(parse_tolerance("1h"), Ok(3600));
        assert_eq!(parse_tolerance("1s"), Ok(1));
    }

    /// Each of these used to load, and each one leaves the real window to whatever a host decides.
    #[test]
    fn a_window_no_host_could_apply_is_not_a_window() {
        for spelling in [
            "banana", // the story's own example: no unit, no number
            "",       // an empty string
            "5",      // a bare number has to guess a unit
            "m",      // a unit with no count
            "5 m",    // whitespace
            "5M",     // the wrong case, so `M` is not silently read as minutes
            "-5m",    // a negative window
            "1.5m",   // a fraction the grammar does not admit
            "1m30s",  // a compound duration the grammar does not admit
            "0s",     // a window of no length rejects every genuine delivery
            "2h",     // longer than a webhook window has any reason to be
            "7d",     // and `d` is not a unit here at all
        ] {
            assert!(
                parse_tolerance(spelling).is_err(),
                "`tolerance = {spelling:?}` must not pass for a window"
            );
        }
    }

    /// Scaling a count into seconds is fallible arithmetic on author input, and it must fail as a
    /// `Result` rather than as a panic or a wrap.
    ///
    /// Both modes are the defect this function exists to close, arrived at from the other end. `*`
    /// panicked inside the loader in a debug build; in a release build it wrapped
    /// `"9223372036854775807m"` to `Ok(-60)` — a negative window that satisfies "not zero" and "not
    /// over an hour", so the declaration **loaded** and shipped a replay bound no host could apply.
    #[test]
    fn a_count_too_large_to_scale_is_refused_rather_than_wrapped() {
        for enormous in [
            // i64::MAX with a unit that has to scale it: `* 60` wrapped to a negative window.
            "9223372036854775807m",
            "9223372036854775807h",
            // The smallest multiple-of-60 overflow, which wrapped to `Ok(-16)`.
            "307445734561825860m",
            // Too large for `i64` before any scaling happens.
            "99999999999999999999s",
            // And the seconds unit, whose scale is 1, still has to clear the hour bound.
            "9223372036854775807s",
        ] {
            let refusal = parse_tolerance(enormous);
            assert!(
                refusal.is_err(),
                "`tolerance = {enormous:?}` must be refused, not wrapped into a window: got \
                 {refusal:?}"
            );
        }

        // The property behind the cases: no accepted window is outside the declared bound, whatever
        // the arithmetic did on the way. A wrap would show up here as a negative `Ok`.
        for digits in ["9223372036854775807", "307445734561825860", "3600", "61"] {
            for unit in ["s", "m", "h"] {
                if let Ok(window) = parse_tolerance(&format!("{digits}{unit}")) {
                    assert!(
                        (1..=MAX_TOLERANCE_SECONDS).contains(&window),
                        "`tolerance = {digits}{unit}` was accepted as {window}s, outside \
                         1..={MAX_TOLERANCE_SECONDS}"
                    );
                }
            }
        }
    }

    /// The one-character typo that would otherwise sign a string with no body in it.
    #[test]
    fn an_unterminated_placeholder_is_reported_as_one_no_host_can_fill() {
        assert_eq!(signed_placeholders("{body"), vec!["{body"]);
        assert_eq!(
            signed_placeholders("v0:{timestamp}:{body"),
            vec!["timestamp", "{body"],
            "a well-formed placeholder before the typo must not hide it — every loader check \
             passes on `[\"timestamp\"]`, and the body then leaves the signed string entirely"
        );
        for placeholder in signed_placeholders("v0:{timestamp}:{body") {
            if placeholder.starts_with('{') {
                assert!(
                    !SIGNED_PLACEHOLDERS.contains(&placeholder.as_str()),
                    "the fragment must be unfillable, so the loader's existing refusal catches it"
                );
            }
        }
    }
}
