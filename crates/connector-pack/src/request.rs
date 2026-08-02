//! The request an operation makes, built from the operation's own emitted Flux.
//!
//! # Why the declaration, and not a second lowering
//!
//! [`crate::spec`] projects the *contract* by reading the shipped `op` declaration back, because two
//! surfaces generated from one IR can drift into disagreeing about the same operation — the risk
//! [the design](../../../docs/designs/connector-tool-pack.md) names first. The **request** is the
//! other half of that risk and it is the more dangerous half: a contract that drifts tells a model
//! something slightly wrong, while a request that drifts sends a vendor a call the module would
//! never have made.
//!
//! So this module does not re-lower `connector_spec`'s IR. It **evaluates the emitted body** — the
//! same statements `connectors/<provider>.flux` carries, the same statements flux itself would run —
//! and reads `{ method, url, headers, body }` off the `http.request` call at the end of it. The
//! pack's request is the module's request by construction, which is the only form of agreement that
//! cannot rot. C-117 asserts it differentially; this is what makes that assertion cheap to satisfy.
//!
//! # The evaluator is deliberately tiny, and deliberately closed
//!
//! `connector-flux` emits exactly one shape ([`connector_flux::op`]'s module documentation states
//! it in full), so the node set this has to understand is small and fixed:
//!
//! | node | where it appears |
//! |---|---|
//! | `Bind` of a `Lit` | `base`, `sep`, `content_type`, a `const`-pinned body field |
//! | `Bind` of a `Fmt` | `url`, and each optional query parameter's re-binding |
//! | `Bind` of an `Obj` | `payload`, nested at the wire paths the emitter honours |
//! | `Bind` of a `Parse` | `payload`, for a free-form body supplied whole by the caller |
//! | `Bind` of a `Call` | `response = http.request(…)` — the request itself |
//! | `When` over a `Var` | the guard that makes an unsupplied filter *not sent* |
//! | `Return` of a `Var` | the end of the body |
//!
//! Anything else is [`Error::Unbuildable`](crate::Error::Unbuildable) rather than ignored. That is
//! the load-bearing choice: an emitter that grows a node this does not model — a `retry`, a quirk
//! compiled into control flow (C-12) — must fail loudly here, because the alternative is a request
//! assembled from *part* of an operation's body and sent anyway. A partly-evaluated request is not a
//! degraded request; it is a different call, and the vendor answers it.
//!
//! # Where the semantics come from
//!
//! Interpolation, truthiness and value-to-text are flux-lang's, reproduced against it rather than
//! invented — `interpolate_str`, `json_truthy` and `lit_text` in flux-lang's `runtime.rs`. The two
//! that matter for a request:
//!
//! - **An unbound `{name}` stays verbatim.** That is why the emitter guards optional query
//!   parameters with `when` instead of interpolating them unconditionally, and reproducing it here
//!   keeps the guard meaningful.
//! - **`null` renders as the empty string**, the `""`-is-absent idiom flux-lang documents on
//!   `lit_text`. A parameter a caller passed `null` for is falsey, so a guarded filter is not sent;
//!   rendering it as the literal text `null` would put `?page=null` on the wire in the one case the
//!   guard exists to prevent.
//!
//! # A `{placeholder}` in a *literal* is the connector's configuration (C-193)
//!
//! flux interpolates `fmt` and never a `lit`, so a brace inside a bound string literal names
//! something flux itself would never fill — which is exactly what a templated `base_url` is. The
//! string literals this module reads braces out of are of **exactly two kinds**, and both are
//! configuration:
//!
//! - a **templated base URL** — a literal carrying `://` — such as the ten across nine connectors
//!   (`{subdomain}.zendesk.com`, `{shop}.myshopify.com`, `{site}.atlassian.net`, freshdesk's and
//!   okta's `{domain}`, `{instance}.my.salesforce.com`, docusign's `{account_host}`/`{account_id}`
//!   pair, contentful's `{space_id}`/`{environment_id}` in two service modules, and statuspage's
//!   `{page_id}`);
//! - a **pin bind** as C-187 added, one per operator-pinned value, each a literal that is nothing
//!   but a placeholder — `zone_id = "{zone_id}"`, `teamId = "{teamId}"`. The emitter binds a `lit`
//!   rather than a value precisely so that this module sees the pin at all.
//!
//! # The two kinds are a **rule**, not an observation (C-232)
//!
//! This paragraph used to end "in the shipped catalogue", with a caveat recording that C-110 had
//! written a third kind — a GraphQL query document, a literal whose braces are the vendor's own
//! selection syntax and not configuration at all. Unconfigured that connector refused every call;
//! configured, substitution rewrote the document. Both are measured in this module's tests, and
//! `docs/designs/graphql-vendors.md` is the finding.
//!
//! A statement about what ships is not a rule: the next connector with a brace in a literal
//! falsifies it again, silently, because nothing executes it. So [`unconfigurable`] classifies every
//! brace-carrying literal into those two kinds and **refuses anything else** —
//! [`Error::Unbuildable`], at projection and again at build, rather than a placeholder invented out
//! of a vendor's syntax. That is the same choice the evaluator above already makes for a node it
//! does not model, applied to the scan.
//!
//! The refusal is not the *repair*. Making a document literal opaque to this scan is C-87 — publish
//! the configuration surface so this reads an operation's variables instead of inferring them from
//! syntax. Until then a connector shaped like C-110's does not install, which is the honest answer
//! and the one an implementor can act on.
//!
//! So [`endpoint_variables`] reads an operation's configuration variables off its own emitted Flux
//! rather than waiting for C-87 to publish them, in the same spirit as everything else here: the
//! pack's request is the module's request by construction. [`Build::endpoints`] then substitutes the
//! host's values, and **into literals only**.
//!
//! Literals only is the safety half. Substituting over the *finished* URL instead would reach a
//! caller's parameter values — an argument spelled `{account_id}` would be filled in with a tenant's
//! configuration on its way to the vendor. A literal is authored by the emitter, so nothing a caller
//! passes can be substituted into. The residual case — a caller value that *contains* a placeholder
//! and therefore survives into the URL — is caught by the guard in [`build`], which refuses a URL
//! still naming a configuration variable rather than sending it.
//!
//! # A value is checked where it is substituted (C-214)
//!
//! Reading the variables off the module is only half of what the module knows. It also knows
//! **where each one lands**, and until C-214 nothing asked: `substitute` filled every brace with
//! whatever the host answered, so a `subdomain` of `acme.zendesk.com@evil.example` composed
//! `https://acme.zendesk.com@evil.example.zendesk.com/…` and the request went to a host the
//! operator never named.
//!
//! [`endpoint_slots`] recovers the position from the same emitted body, by the same principle as
//! everything else in this module — the catalogue does not carry a connector's `binds` targets
//! (that is C-87), and waiting for it would leave the guard unbound for another release. A value is
//! then checked against its [`Slot`] at the one substitution point, so the rule binds every host and
//! every `ConfigStore` rather than the loader's view of an `example`. See [`Slot`] for what each
//! position refuses and why the three answers differ.

use std::collections::{BTreeMap, BTreeSet};

use flux_lang::ast::Node;
use flux_lang::program::CompositeOpDecl;
use serde_json::Value;

use crate::Error;

/// The op the whole pack delegates to. Named once so the check below is a comparison rather than a
/// scattered string.
const HTTP_REQUEST: &str = "http.request";

/// The header this software identifies itself in.
const USER_AGENT: &str = "User-Agent";

/// **What this software calls itself on the wire** (C-223).
///
/// A product token and its version, per RFC 9110 §10.1.5, with the repository as the comment a
/// vendor can act on. It names *this* software rather than a browser or a bare product word, which
/// is the acceptance the story states in the form it matters: a `User-Agent` that lies is worse than
/// one that is absent, because a vendor's rate limit, allow-list and support desk all believe it.
///
/// Both halves are read from the manifest rather than typed, so neither can go stale at a release:
/// `CARGO_PKG_VERSION` is the workspace version every crate here inherits, and
/// `CARGO_PKG_REPOSITORY` is the `repository` field the publishing contract already requires. The
/// concatenation is `const`, so this costs nothing at runtime and is one `&'static str` a test can
/// compare against.
pub const DEFAULT_USER_AGENT: &str = concat!(
    "flux-connectors/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

/// **The request**: `{ method, url, headers, body }`, exactly `http.request`'s own input.
///
/// A typed value rather than a bare `serde_json::Value` so a test can assert on the pieces that go
/// wrong silently — a flattened body, a missing `?`/`&` separator — instead of on a blob.
/// [`Request::to_params`] is the one place it becomes the JSON `http.request` reads.
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    /// The HTTP method, as `http.request` spells it (`GET`, `PUT`, …).
    pub method: String,
    /// The absolute request URL, query string included.
    pub url: String,
    /// The request headers. `BTreeMap` because the emitted record is one, so the order is the
    /// module's order rather than a hash seed's.
    pub headers: BTreeMap<String, String>,
    /// The request body as the text `http.request` sends, or `None` for a request that has none.
    ///
    /// Text rather than a `Value` because that is what actually travels: `http.request` reads its
    /// `body` argument with `Value::as_str`, and the emitter binds the payload to a symbol
    /// precisely so flux stores it as canonical JSON *text* rather than handing over an object that
    /// would be dropped without a word.
    pub body: Option<String>,
}

/// **Hand-written, and no value prints** (C-159, finding 1).
///
/// A `Request` carries the assembled credential *after* `auth::place` has run — in a header value,
/// and for a query placement in the URL itself — and this type is `pub`, so a host can hold one and
/// format it. C-152 hand-wrote a redacting `Debug` for `auth::Assembled` for
/// exactly this reason, and the review that closed it observed that this is the **larger** of the
/// two exposures: `Assembled` is constructed at one internal site and never escapes, while this is
/// public API.
///
/// The rule is **shape without values**, which is what a `Debug` of a request is read for: the
/// method, the host, the path, the header *names* and the query-parameter *names* stay, and every
/// value is `<redacted>`. Nothing here tries to work out which header holds the credential — a
/// request cannot know, and an allow-list of "safe" header names is a list that rots into a leak the
/// first time a vendor puts a token somewhere new.
///
/// A body prints as present or absent and never as content, and never as a length: a length is a
/// fingerprint, which is the care [`connector_secrets::Secret`]'s own `Debug` already takes. No
/// placement puts a credential in a body today, so this is the same foot-gun argument the derive
/// itself lost — the value is caller data, and the only thing a reader needs from it here is whether
/// one was built at all.
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &redacted_url(&self.url))
            .field("headers", &HeaderNames(&self.headers))
            .field("body", &self.body.as_ref().map(|_| Redacted))
            .finish()
    }
}

/// A value that does not print. A type rather than a `format_args!` at each site, because the header
/// map needs it as a `Debug` *value* inside an entry and `Option::map` needs it as one too.
struct Redacted;

impl std::fmt::Debug for Redacted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// The headers as their names, every value [`Redacted`].
struct HeaderNames<'a>(&'a BTreeMap<String, String>);

impl std::fmt::Debug for HeaderNames<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.0.keys().map(|name| (name, Redacted)))
            .finish()
    }
}

/// A URL with every query-parameter *value* redacted and every parameter *name* kept.
///
/// The cut is at the `?` because that is where the two kinds of data separate: everything before it
/// is the emitter's own — a host and a path, the facts a reader is chasing — and everything after it
/// is values, one of which may be a [`Placement::Query`](catalog::Placement::Query) credential.
///
/// Anything in the query that is not a `name=value` pair is redacted whole rather than reasoned
/// about. Fail-closed costs one word of debugging output in a case the emitter does not produce.
fn redacted_url(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_owned();
    };
    let mut out = String::with_capacity(url.len());
    out.push_str(base);
    out.push('?');
    for (index, pair) in query.split('&').enumerate() {
        if index > 0 {
            out.push('&');
        }
        match pair.split_once('=') {
            Some((name, _)) => {
                out.push_str(name);
                out.push_str("=<redacted>");
            }
            None => out.push_str("<redacted>"),
        }
    }
    out
}

impl Request {
    /// The params `http.request` is called with.
    ///
    /// `headers` and `body` are omitted when empty rather than sent as `{}`/`""`, so a request this
    /// pack builds is the same JSON a hand-written `http.request` call would carry.
    pub fn to_params(&self) -> Value {
        let mut params = serde_json::Map::new();
        params.insert("url".to_string(), Value::String(self.url.clone()));
        params.insert("method".to_string(), Value::String(self.method.clone()));
        if !self.headers.is_empty() {
            params.insert(
                "headers".to_string(),
                Value::Object(
                    self.headers
                        .iter()
                        .map(|(name, value)| (name.clone(), Value::String(value.clone())))
                        .collect(),
                ),
            );
        }
        if let Some(body) = &self.body {
            params.insert("body".to_string(), Value::String(body.clone()));
        }
        Value::Object(params)
    }
}

/// **The endpoint-configuration variables `declaration` needs**, in stable order.
///
/// Every `{name}` appearing in a string literal of one of the **two declared kinds** — a templated
/// URL or a pin bind. See this module's documentation for why those two are exactly the connector's
/// configuration and nothing else: flux interpolates `fmt` and never `lit`, so a brace surviving in
/// a literal of either shape is by construction a name no evaluation fills.
///
/// A brace-carrying literal that is neither kind contributes **nothing** here and is refused
/// outright by [`refuse_unconfigurable`] instead. That ordering is the C-232 fix: a variable this
/// function reports is a variable a `[[config]]` field declares, so a test that binds what this
/// reports is no longer binding whatever the scan happened to find.
///
/// Derived once, when the operation is projected, so a missing value is a refusal that can name the
/// variable *before* anything is assembled — rather than a brace noticed in a finished URL.
pub(crate) fn endpoint_variables(declaration: &CompositeOpDecl) -> Vec<String> {
    let mut found = BTreeSet::new();
    walk_literals(&declaration.body.body, &mut |literal| {
        if unconfigurable(literal).is_some() {
            return;
        }
        scan_template(literal, |name| {
            found.insert(name.to_owned());
            None
        });
    });
    found.into_iter().collect()
}

/// **A brace-carrying literal that is neither of the two declared kinds**, or `None`.
///
/// The classification is deliberately the same pair of tests the rest of the module already makes,
/// rather than a third opinion about what a placeholder is: [`sole_placeholder`] is what
/// [`endpoint_slots`] calls a pin bind, and `://` is what [`place_in_url`] splits a templated URL
/// on. Two spellings of "is this a URL" would eventually disagree, and the one that disagreed would
/// be the one deciding whether a vendor's own syntax gets filled with a tenant's settings.
fn unconfigurable(literal: &str) -> Option<&str> {
    if !carries_placeholder(literal) {
        return None;
    }
    if sole_placeholder(literal).is_some() || literal.contains("://") {
        return None;
    }
    Some(literal)
}

/// Whether `literal` carries a `{…}` this module would read as a placeholder at all.
///
/// Asked through [`scan_template`] rather than by looking for a `{`, so an unterminated brace or a
/// `{{` escape is answered exactly as substitution would answer it.
fn carries_placeholder(literal: &str) -> bool {
    let mut any = false;
    scan_template(literal, |_| {
        any = true;
        None
    });
    any
}

/// **Refuse an operation whose body binds a literal this module cannot classify** (C-232).
///
/// Called at projection ([`crate::Operation::project`]) and again at [`build`]. Twice, because they
/// are two entry points a host can reach and the answer must not depend on which: projection is
/// where a connector fails to install, and `build` is the one gate every request passes through.
///
/// **Both call sites are pinned**, which is worth saying because the doubling is the kind of claim
/// that quietly becomes a single check: deleting the call in `crate::Operation::project` fails
/// `tests/request.rs::a_document_literal_is_refused_at_projection_and_not_only_at_build`, and
/// deleting the one below fails
/// [`tests::a_graphql_operation_is_refused_rather_than_configured_or_corrupted`].
///
/// # Errors
///
/// [`Error::Unbuildable`], quoting the literal so an implementor can find it in their own emitted
/// module, and naming **which** clause refused — a document whose braces are the vendor's own
/// syntax, or a sole placeholder whose name [`is_pin_name`] does not model. The two want opposite
/// responses (rewrite the operation; rename the field), so collapsing them into one sentence would
/// send whoever hits the second one looking in the wrong place.
pub(crate) fn refuse_unconfigurable(
    operation: &str,
    declaration: &CompositeOpDecl,
) -> Result<(), Error> {
    let mut refused = None;
    walk_literals(&declaration.body.body, &mut |literal| {
        if refused.is_none() {
            refused = unconfigurable(literal).map(str::to_owned);
        }
    });
    match refused {
        None => Ok(()),
        Some(literal) => Err(Error::Unbuildable {
            operation: operation.to_owned(),
            message: unconfigurable_reason(&literal),
        }),
    }
}

/// Why `literal` could not be classified, phrased for whoever has to change something.
fn unconfigurable_reason(literal: &str) -> String {
    let sole = literal
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .map(str::trim)
        .filter(|inner| !inner.is_empty() && !inner.contains(['{', '}']));
    match sole {
        // A pin the *loader* would accept and this module does not model. Narrower than the loader
        // on purpose — see [`is_pin_name`] — so the answer is "rename the field", not "this is not a
        // pin", which is what an earlier revision said and would have sent someone hunting.
        Some(name) => format!(
            "its body binds the pin literal {}, and {name:?} is not a name this pack models: a \
             configuration pin may not carry whitespace or a `\"`. The loader is wider — it \
             requires only that a `path.`/`query.`/`header.` binding name something — so this is \
             this pack's limit and not a defect in the provider file. Renaming the `[[config]]` \
             field's `binds` suffix is the fix",
            abbreviated(literal)
        ),
        None => format!(
            "its body binds the string literal {}, whose `{{…}}` is neither a templated URL nor a \
             configuration pin — the only two kinds this pack reads a brace in a literal as \
             (C-193). Filling it would substitute a tenant's configuration into the vendor's own \
             syntax, which is what withdrew C-110's connector; making a literal opaque to this scan \
             is C-87, publishing the configuration surface",
            abbreviated(literal)
        ),
    }
}

/// A literal, quoted and cut to a length a refusal can carry.
///
/// The cut is on a `char` boundary and the ellipsis is outside the quotes, so what is printed is
/// never mistaken for the literal itself.
fn abbreviated(literal: &str) -> String {
    const KEEP: usize = 100;
    match literal.char_indices().nth(KEEP) {
        None => format!("{literal:?}"),
        Some((end, _)) => format!("{:?}…", &literal[..end]),
    }
}

/// Visit every string literal in `nodes`.
///
/// The node set walked is exactly the one [`run`] and [`eval`] model; anything outside it is
/// [`Error::Unbuildable`] at build time, so skipping it here cannot let a request through with a
/// variable nobody resolved.
fn walk_literals(nodes: &[Node], visit: &mut dyn FnMut(&str)) {
    for node in nodes {
        walk_literals_node(node, visit);
    }
}

fn walk_literals_node(node: &Node, visit: &mut dyn FnMut(&str)) {
    match node {
        Node::Lit {
            value: Value::String(text),
        } => visit(text),
        Node::Bind { value, .. } => walk_literals_node(value, visit),
        Node::Call { args, .. } => walk_literals(args, visit),
        Node::Obj { fields } => {
            for value in fields.values() {
                walk_literals_node(value, visit);
            }
        }
        Node::List { items } => walk_literals(items, visit),
        Node::Parse { value, .. } => walk_literals_node(value, visit),
        Node::When {
            cond,
            then,
            otherwise,
        } => {
            walk_literals_node(cond, visit);
            walk_literals(then, visit);
            walk_literals(otherwise, visit);
        }
        // `Var`, `Fmt` and `Return` carry no literal text: a `fmt` template's placeholders are
        // symbols the body binds, which is the opposite of what this is looking for.
        _ => {}
    }
}

/// **Where a configuration variable lands on the request** — and therefore what its value may be.
///
/// The three request positions need three answers, and "percent-encode it" is the wrong answer to
/// two of them. Encoding a *path segment* is meaningful; encoding a **host** is not, because a host
/// has different legal syntax and a different failure mode, and there is no encoding at all for an
/// HTTP field value — a bad one can only be refused.
///
/// | slot | answer | why |
/// |---|---|---|
/// | [`Host`](Self::Host) | refuse unless the whole composed authority is a hostname | a value here moves the **origin**; see [`validate_authority`] |
/// | [`Path`](Self::Path) | refuse | a `zone_id` with a `/` in it is an operator's mistake, and silently encoding it produces a 404 they cannot diagnose |
/// | [`Query`](Self::Query) | refuse the structural characters, then percent-encode with [`crate::auth::query_encode`] | the encoder already exists and is the identity over unreserved characters; a second one would put two spellings on one URL |
/// | [`Header`](Self::Header) | refuse | a CR or LF appends a header of the value's choosing, and no encoding exists to make it safe |
/// | [`Unplaced`](Self::Unplaced) | refuse unless **every** rule above accepts it, the host rule included | fail closed |
///
/// # This is a second spelling of `connector_spec::Position::validate_value`, deliberately
///
/// C-214 asks for that predicate to be *reused* rather than reimplemented — "two spellings of one
/// rule is the defect this story is an instance of" — and it takes the story's sanctioned
/// alternative instead: replaced, with the reason recorded. Three facts, in the order they matter.
///
/// **Reuse could never have covered this type.** `connector_spec::Position` is a closed set of the
/// three positions a *pinned* value lands on, and it has **no `Host` variant** — a templated host is
/// `Binding::Endpoint`, a different binding asking a different question. So [`validate_authority`]
/// had to be written here whatever else happened, and reuse could have covered at best three of
/// these five slots: the severe half of this story, the half that predates C-187, was never
/// reusable. That is the argument, and it stands on its own.
///
/// **The edge does not exist.** Every `connector_spec` name in `connector-pack` today is prose in a
/// doc comment; the pack's input is the *catalogue*, which carries no `binds` target and no
/// `Position` (that is C-87). Adding it is a manifest and lockfile change, both outside what this
/// change is allowed to touch.
///
/// **It is not forbidden, and this comment previously implied it was.** `AGENTS.md`'s dependency
/// fence is *directional* — it forbids the compiler crates from reaching the host and network side,
/// and `connector-pack` → `connector-spec` is the opposite direction, unguarded. `connector-spec` is
/// already in the publish closure, so the edge would not enlarge it either. The reason to weigh the
/// edge carefully is the `Host` argument above plus the design intent this module opens with, not a
/// rule that would refuse it.
///
/// # Where the two spellings actually differ
///
/// Not "exact" — an earlier revision of this comment claimed that, and it was wrong in a way someone
/// would have trusted instead of re-measuring. Against
/// `crates/connector-spec/src/config.rs:269-333`, the path and header rules match character for
/// character. The query rule differs by exactly one character:
///
/// | | refuses in a query value |
/// |---|---|
/// | `connector-spec` | `&` `=` `?` `#` `+` **`%`**, whitespace, control |
/// | here ([`validate_query`]) | `&` `=` `?` `#` `+`, whitespace, control |
///
/// **The difference is principled rather than accidental, and it is safe in this direction.** The
/// loader's own stated reason for refusing `%` is that "nothing percent-encodes a query value on the
/// way out" — true where it runs, because a declared `example` is never encoded by anything. Here a
/// value *is* encoded: [`crate::auth::query_encode`] maps `%` to `%25` along with every other
/// non-unreserved byte, so `50%off` travels as `50%25off` and cannot become an escape the vendor
/// re-reads. Refusing it here would reject a legitimate value for a hazard the encoder removes.
///
/// The asymmetry runs the **fail-safe way**: the loader is the stricter of the two, so a provider
/// author cannot ship `example = "50%off"` for a query pin, while a tenant may still supply one. A
/// drift in the other direction — runtime stricter than loader — would ship an example no tenant
/// could use, and that is the one worth a test if these ever move.
///
/// **Unifying the two behind one crate is the follow-up**, and unification has to settle this
/// divergence rather than paper over it: one rule cannot both know about an encoder and not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    /// The authority of a templated base URL — the `{subdomain}` of
    /// `https://{subdomain}.zendesk.com`. The severe one, and the one that predates C-187.
    Host,
    /// A path segment: a `{var}` in the path half of a base URL (docusign's `{account_id}`,
    /// contentful's `{space_id}`), or a `Position::Path` pin (cloudflare's `{zone_id}`).
    Path,
    /// A query parameter value — a `Position::Query` pin, such as vercel's `{teamId}`.
    Query,
    /// A request header value — a `Position::Header` pin. No shipped provider declares one; C-164
    /// will be the first, which is why the guard is proved against a fixture.
    Header,
    /// **A placeholder this derivation could not place in exactly one position** — either because
    /// the emitter produced a shape it does not model, or because the value genuinely lands in more
    /// than one (C-229).
    ///
    /// It exists so that an emitter which grows a shape this does not model degrades into *more*
    /// refusal rather than into none, which is the same choice [`Error::Unbuildable`] makes for the
    /// evaluator, applied to the guard. Its rule is therefore the **intersection of every position's
    /// rule**, the host's included, and it does not encode: an encoding is only safe where the
    /// position is known.
    ///
    /// # C-229 made it reachable, deliberately, and this is the decision rather than an inheritance
    ///
    /// `providers/algolia.toml` declares one `[[config]]` field whose value is both the `{app_id}`
    /// of `https://{app_id}.algolia.net` and the `X-Algolia-Application-Id` header on every call, so
    /// [`record`] sees one variable in two positions and collapses it here. **That is the right
    /// answer rather than a degradation**, and it is the answer the story asks for in as many words:
    /// a value reaching a hostname and a header must satisfy *both* predicates, the host predicate
    /// is the strict one, and a value legal in a header and illegal in a hostname is refused rather
    /// than encoded differently per destination. This arm is exactly that — every rule at once, one
    /// substituted string.
    ///
    /// The name is now half a description: `algolia/app_id` is not unplaceable, it is *multiply*
    /// placed. Renaming it to something like `Everywhere` was weighed and left alone, because the
    /// two cases share one rule and one reason for it, and a second variant carrying the identical
    /// arm would be two spellings of one answer — the defect this repository keeps writing stories
    /// about. What the two cases do not share is how they are audited, and that is
    /// [`tests::every_shipped_configuration_variable_is_placed`]'s job: it names the multiply-placed
    /// variables explicitly, so an *accidental* arrival here is still a red test.
    Unplaced,
}

impl Slot {
    /// The word a refusal calls this position by.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Path => "path segment",
            Self::Query => "query parameter",
            Self::Header => "header",
            Self::Unplaced => "unplaced position",
        }
    }

    /// **Whether `value` may be substituted here at all** — for the one caller that cannot fail.
    ///
    /// `Operation::subjects` has nowhere to put a refusal, so it needs the predicate without the
    /// error. For [`Host`](Self::Host) it applies [`validate_authority`] to the value alone, which
    /// is a sufficient condition rather than the full check: a value made only of host characters
    /// cannot introduce a delimiter into a template that had none either.
    pub(crate) fn substitutable(self, value: &str) -> bool {
        match self {
            Self::Host => validate_authority(value).is_ok(),
            other => other.validate(value).is_ok(),
        }
    }

    /// **Whether `value` can be substituted here without reshaping the request** — and the text to
    /// substitute if it can.
    ///
    /// The return is the substituted text rather than `()` because exactly one position has an
    /// encoder: a query value goes through [`crate::auth::query_encode`], which is the identity over
    /// RFC 3986's unreserved set and therefore changes nothing for any value a vendor would call a
    /// team or a zone. Every other position substitutes the value unchanged or refuses it.
    ///
    /// [`Host`](Self::Host) is not answered here. Its question is about the authority the *template*
    /// composes rather than about the value alone, so it is [`validate_authority`]'s, and this
    /// returns the value untouched for [`Build::check_authority`] to judge in context.
    ///
    /// See this type's documentation for why these rules are spelled here rather than reused from
    /// `connector_spec::Position::validate_value`.
    ///
    /// # Errors
    ///
    /// The reason, phrased for the operator who supplied the value.
    pub(crate) fn validate(self, value: &str) -> Result<String, String> {
        if value.trim().is_empty() {
            return Err("a configuration value must not be empty or all whitespace".to_owned());
        }
        // Refused in every position: substitution fills `{placeholder}`s in emitted literals, so a
        // value spelling one would either be filled in a second time or survive into the URL as
        // text — which is what `Error::UnresolvedEndpoint` would then refuse, one step too late to
        // say anything useful about the value.
        if let Some(bad) = value.chars().find(|c| *c == '{' || *c == '}') {
            return Err(format!(
                "{value:?} contains {bad:?}, and a configuration value is substituted into a \
                 `{{placeholder}}`, so a value spelling one of its own would be filled in twice or \
                 reach the vendor verbatim"
            ));
        }
        match self {
            Self::Host => Ok(value.to_owned()),
            Self::Path => {
                validate_path(value)?;
                Ok(value.to_owned())
            }
            Self::Query => {
                validate_query(value)?;
                Ok(crate::auth::query_encode(value))
            }
            Self::Header => {
                validate_header(value)?;
                Ok(value.to_owned())
            }
            // Fail closed: a placeholder nothing placed is held to **every** rule at once, the host
            // rule included, and is not encoded — an encoding is only safe where the position is
            // known.
            //
            // The host rule is load-bearing here rather than decorative. Without it this arm
            // accepted `acme.zendesk.com@evil.example`: neither `@` nor `:` appears in the path,
            // query or header charsets, because none of those three positions cares about them.
            // Only the host does, and the host is the one position an unplaced value could be
            // sitting in. A defence-in-depth layer that omits the defence it exists for is worse
            // than none, because the comment above it gets believed.
            Self::Unplaced => {
                validate_path(value)?;
                validate_query(value)?;
                validate_header(value)?;
                validate_authority(value)?;
                Ok(value.to_owned())
            }
        }
    }
}

/// A value that stays inside one path segment.
fn validate_path(value: &str) -> Result<(), String> {
    if value == "." || value == ".." {
        return Err(format!(
            "{value:?} is a relative path segment, so the request would address the segment above \
             or beside the one it was configured for"
        ));
    }
    match value
        .chars()
        .find(|c| "/?#%\\".contains(*c) || c.is_whitespace() || c.is_control())
    {
        Some(bad) => Err(format!(
            "{value:?} contains {bad:?}, which does not stay inside one path segment — a `/` (or a \
             `%` that could encode one) reshapes the URL, and a `?` or `#` ends the path entirely"
        )),
        None => Ok(()),
    }
}

/// A value that adds no parameter of its own to the query string.
///
/// Refused *before* [`crate::auth::query_encode`] runs rather than left to it: encoding an `&` would
/// send `%26` where the operator plainly meant a separator, which is a request the vendor answers
/// with something confusing instead of a refusal they can act on. The encoder then covers the
/// remainder — `/`, `:`, `@`, a comma — which is safe to encode because it has one obvious meaning.
fn validate_query(value: &str) -> Result<(), String> {
    match value
        .chars()
        .find(|c| "&=?#+".contains(*c) || c.is_whitespace() || c.is_control())
    {
        Some(bad) => Err(format!(
            "{value:?} contains {bad:?}, which is query-string structure rather than a value — an \
             `&` or `=` would add a parameter of its own to every request this connector makes"
        )),
        None => Ok(()),
    }
}

/// A value that is an HTTP field value and nothing more (RFC 9110 §5.5).
///
/// The one position here with a classic exploit: a raw CR or LF appends a header of the value's
/// choosing to every request the service makes. No shipped provider declares a header pin yet —
/// C-164's Algolia will be the first — so this is proved against a fixture rather than against the
/// catalogue.
fn validate_header(value: &str) -> Result<(), String> {
    if let Some(bad) = value
        .chars()
        .find(|c| !c.is_ascii() || c.is_ascii_control())
    {
        return Err(format!(
            "{value:?} contains {bad:?}, which is not an HTTP field value (RFC 9110 §5.5) — a \
             newline in particular would append a header of its own to every request"
        ));
    }
    if value.trim() != value {
        return Err(format!(
            "{value:?} starts or ends with whitespace, which an HTTP field value may not \
             (RFC 9110 §5.5)"
        ));
    }
    Ok(())
}

/// **Whether `authority` is still an authority once a configuration value has been substituted into
/// it.**
///
/// `authority` is the authority the declaration implies, with every `{var}` filled — for
/// `https://{subdomain}.zendesk.com` and a value of `acme`, the string `acme.zendesk.com`. It must
/// consist only of characters that **cannot delimit an authority**: ASCII alphanumerics, `-`, `.`
/// and `_`. Every dot-separated label must be non-empty.
///
/// # Why an allow-list, and not a blocklist of `@`, `/` and `:`
///
/// The measured case is
///
/// ```text
/// subdomain = "acme.zendesk.com@evil.example"
///   -> https://acme.zendesk.com@evil.example.zendesk.com/api/v2/tickets/1
///      authority: evil.example.zendesk.com
/// ```
///
/// where the `@` turns everything before it into userinfo, and the request goes to a host the
/// operator never named carrying that operator's own token, through an egress gate that was shown a
/// subject ending in `zendesk.com`. A blocklist stops that one. An allow-list also stops the ones
/// nobody enumerated — a `:` that becomes a port, a `%` that decodes to a delimiter, a `/` that ends
/// the authority early, whitespace, a control character, and any non-ASCII codepoint an IDNA mapping
/// could fold onto a different label.
///
/// Because no permitted character can delimit, the string a transport resolves is **exactly** this
/// string. The composed authority therefore still ends in whatever fixed suffix the template
/// declared — the property C-214 asks to be compared — and it holds as a consequence of the rule
/// rather than as a second rule that could disagree with it.
///
/// `_` is permitted because DNS carries it and no URL parser treats it as a delimiter, so it cannot
/// move an origin. Case is not folded, because hostnames are case-insensitive and refusing
/// `Acme.zendesk.com` would be strictness with nothing behind it.
///
/// # Errors
///
/// The reason, phrased for the operator who supplied the value.
fn validate_authority(authority: &str) -> Result<(), String> {
    if authority.is_empty() {
        return Err("a host must not be empty".to_owned());
    }
    if let Some(bad) = authority
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '.' || *c == '_'))
    {
        return Err(format!(
            "the composed host {authority:?} contains {bad:?}, which is not a host character — a \
             configuration value may not introduce one, because `@`, `:`, `/` and `%` all move or \
             truncate the authority the connector declared"
        ));
    }
    if authority.split('.').any(str::is_empty) {
        return Err(format!(
            "the composed host {authority:?} has an empty label, so it is not a hostname"
        ));
    }
    Ok(())
}

/// Validate an authority whose **template**, rather than a configured value, may state a port.
///
/// Asterisk ARI is the first shipped case: `https://{host}:8089/ari`. The existing host predicate
/// correctly refuses `host = "evil.example:8089"`, because a configured colon can redirect a
/// request to a different port. Applying that same predicate to the finished authority also refused
/// the emitter-authored `:8088`, though no configured byte introduced it. This split retains the
/// safety property: only one decimal port written literally in the connector is admitted, while
/// every substituted host value remains subject to [`validate_authority`].
fn validate_templated_authority(template: &str, composed: &str) -> Result<(), String> {
    let Some((_template_host, template_port)) = template.rsplit_once(':') else {
        return validate_authority(composed);
    };
    if template_port.contains(MARK)
        || template_port.is_empty()
        || !template_port.chars().all(|c| c.is_ascii_digit())
    {
        return validate_authority(composed);
    }
    let port: u16 = template_port
        .parse()
        .map_err(|_| format!("the declared port {template_port:?} is not between 1 and 65535"))?;
    if port == 0 {
        return Err("the declared port 0 is not between 1 and 65535".to_owned());
    }
    let Some((composed_host, composed_port)) = composed.rsplit_once(':') else {
        return Err(format!(
            "the template declares port {template_port}, but the composed authority {composed:?} does not"
        ));
    };
    if composed_port != template_port {
        return Err(format!(
            "the template declares port {template_port}, but the composed authority uses {composed_port:?}"
        ));
    }
    // The composed host half still includes every substituted value; validating it proves no
    // configured value introduced a delimiter. The template was read only to establish that the
    // port was literal.
    validate_authority(composed_host)
}

/// The sentinel that stands in for a placeholder while a template is being read positionally.
///
/// `\u{1}` because a template is emitter-authored text and carries no control characters. **A
/// configuration value never passes through a marked string** — markers are resolved against the
/// template alone, and values are substituted only into the result — so a value cannot forge one.
const MARK: char = '\u{1}';

/// Wrap `name` as a positional marker.
fn mark(name: &str) -> String {
    format!("{MARK}{name}{MARK}")
}

/// **Where each of `declaration`'s configuration variables lands**, read off the emitted body.
///
/// Derived once, when the operation is projected, alongside [`endpoint_variables`] — and from the
/// same source, for the same reason. A variable this cannot place is simply absent from the map,
/// and [`Build::check`] reads an absent entry as [`Slot::Unplaced`], which refuses the most.
///
/// The derivation follows the two literal shapes the emitter produces (see this module's
/// documentation):
///
/// 1. a literal that is **nothing but a placeholder** is a pin bind, and its position is decided by
///    where the *symbol* it binds is used — in the URL's `fmt` template before the `?` (path) or
///    after it (query), or as a header record value (header);
/// 2. any other literal carrying a brace is a templated URL, and a placeholder's position is
///    decided by where it falls in that URL — inside the authority (host) or after it (path/query).
pub(crate) fn endpoint_slots(declaration: &CompositeOpDecl) -> BTreeMap<String, Slot> {
    let body = &declaration.body.body;
    let mut slots = BTreeMap::new();
    // Symbol -> the variable a pin bind holds, and symbol -> literal text for every string literal.
    // The second is what expands `{base}` inside the URL template so the `?` scan reads the whole
    // URL rather than the tail of it.
    let mut pins = BTreeMap::new();
    let mut literals = BTreeMap::new();

    collect_binds(body, &mut pins, &mut literals, &mut slots);
    place_pins(body, &pins, &literals, &mut slots);
    slots
}

/// **Caller parameters that the emitted declaration places in a URL path segment.**
///
/// This is deliberately read from the Flux body rather than from catalogue IR or parameter names:
/// the pack executes the declaration, so the declaration is the only source that cannot disagree
/// with the request. Every `fmt` is visited recursively through both arms of a [`Node::When`], which
/// matters for a guarded path extension even when the current call does not take that branch.
///
/// Literal symbols are expanded before placement. In particular, the emitter's `sep` alternates
/// between `?` and `&`; treating that closed pair as a query boundary keeps guarded query arguments
/// out of the path set without evaluating one caller's branch choices at projection time.
pub(crate) fn caller_path_parameters(declaration: &CompositeOpDecl) -> BTreeSet<String> {
    let parameters: BTreeSet<&str> = declaration
        .params
        .iter()
        .map(|parameter| parameter.name.0.as_str())
        .collect();
    let mut literals: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    collect_literal_alternatives(&declaration.body.body, &mut literals);

    let mut path = BTreeSet::new();
    collect_caller_path_parameters(&declaration.body.body, &parameters, &literals, &mut path);
    path
}

/// Every literal spelling a symbol may have across guarded branches.
fn collect_literal_alternatives(nodes: &[Node], literals: &mut BTreeMap<String, BTreeSet<String>>) {
    for node in nodes {
        match node {
            Node::Bind { name, value, .. } => {
                if let Node::Lit {
                    value: Value::String(text),
                } = value.as_ref()
                {
                    literals
                        .entry(name.0.clone())
                        .or_default()
                        .insert(text.clone());
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                collect_literal_alternatives(then, literals);
                collect_literal_alternatives(otherwise, literals);
            }
            _ => {}
        }
    }
}

/// Locate caller symbols in every interpolation the emitted body can evaluate.
fn collect_caller_path_parameters(
    nodes: &[Node],
    parameters: &BTreeSet<&str>,
    literals: &BTreeMap<String, BTreeSet<String>>,
    path: &mut BTreeSet<String>,
) {
    for node in nodes {
        match node {
            Node::Bind { value, .. } => {
                if let Node::Fmt { template } = value.as_ref() {
                    place_caller_parameters(template, parameters, literals, path);
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                collect_caller_path_parameters(then, parameters, literals, path);
                collect_caller_path_parameters(otherwise, parameters, literals, path);
            }
            _ => {}
        }
    }
}

/// Place caller markers relative to the URL's first query/fragment delimiter.
fn place_caller_parameters(
    template: &str,
    parameters: &BTreeSet<&str>,
    literals: &BTreeMap<String, BTreeSet<String>>,
    path: &mut BTreeSet<String>,
) {
    let expanded = scan_template(template, |symbol| {
        if parameters.contains(symbol) {
            return Some(mark(symbol));
        }
        let alternatives = literals.get(symbol)?;
        if alternatives.len() == 1 {
            return alternatives.first().cloned();
        }
        // `sep` is `?` before the first included query argument and `&` thereafter. Both place
        // everything following them in the query, so `?` is the stable positional representative.
        (alternatives.contains("?")
            && alternatives
                .iter()
                .all(|literal| matches!(literal.as_str(), "?" | "&")))
        .then(|| "?".to_owned())
    });
    let query_start = expanded.find(['?', '#']).unwrap_or(expanded.len());
    for (offset, parameter) in marked_placeholders(&expanded) {
        if offset < query_start {
            path.insert(parameter.to_owned());
        }
    }
}

/// Walk every statement, recording pin binds, literal binds, and the placements a URL literal
/// settles on its own.
fn collect_binds(
    nodes: &[Node],
    pins: &mut BTreeMap<String, String>,
    literals: &mut BTreeMap<String, String>,
    slots: &mut BTreeMap<String, Slot>,
) {
    for node in nodes {
        match node {
            Node::Bind { name, value, .. } => {
                if let Node::Lit {
                    value: Value::String(text),
                } = value.as_ref()
                {
                    literals.insert(name.0.clone(), text.clone());
                    match sole_placeholder(text) {
                        Some(variable) => {
                            pins.insert(name.0.clone(), variable.to_owned());
                        }
                        None => place_in_url(text, slots),
                    }
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                collect_binds(then, pins, literals, slots);
                collect_binds(otherwise, pins, literals, slots);
            }
            _ => {}
        }
    }
}

/// The single variable `literal` consists of, if it is nothing else — the pin-bind shape
/// `zone_id = "{zone_id}"`.
///
/// The inner text must be a **pin name**, not merely brace-delimited. Without that clause
/// `{"already": "json"}` — a JSON object bound as a literal — reads as a pin named
/// `"already": "json"`, and a shape this module cannot classify would slip through as one it can.
fn sole_placeholder(literal: &str) -> Option<&str> {
    let inner = literal.strip_prefix('{')?.strip_suffix('}')?;
    let inner = inner.trim();
    is_pin_name(inner).then_some(inner)
}

/// **Whether `name` can be the `binds` suffix of a pin, as this module is able to recognise one.**
///
/// # The loader is authoritative, and this is deliberately narrower than it
///
/// `connector_spec::config`'s `parse_binding` requires a `path.`/`query.`/`header.` suffix to be
/// **non-empty and nothing else** (`crates/connector-spec/src/config.rs:465-480`), so the loader is
/// the wider grammar and the one a provider author is actually held to. An earlier revision of this
/// comment claimed the two were the same rule; they are not, and the difference is measurable:
/// spelling `providers/vercel.toml`'s pin `binds = "query.page.size"` loads, emits
/// `page_size = "{page.size}"`, and an identifier-only predicate then reports it as *"neither a
/// templated URL nor a configuration pin"* — a wrong diagnosis for a literal that is exactly a pin.
///
/// So the reconciliation runs toward the loader, and stops one clause short of it: **a pin name
/// carries no whitespace and no `"`.** Dotted, bracketed and hyphenated names — `page.size`,
/// `page[size]`, `a-b` — are pins here exactly as they are for the loader.
///
/// The one clause is load-bearing rather than tidy-minded, and it is what the quote is doing:
/// **a JSON object literal necessarily quotes its keys**, so `{"already": "json"}` and every
/// minified sibling of it fails this and is refused as the unclassifiable literal it is. Dropping
/// the clause to match the loader exactly would re-admit that shape as a pin named after its own
/// contents, which is the C-110 failure with a different vendor syntax.
///
/// A pin whose declared name *does* carry whitespace or a quote is therefore refused here while the
/// loader accepts it. That is fail-closed and no shipped provider is affected, and
/// [`refuse_unconfigurable`] says which of the two clauses refused so the diagnosis is right when it
/// happens. [`tests::the_pin_name_rule_is_narrower_than_the_loaders`] is where the divergence is
/// pinned, in the same spirit as
/// [`tests::the_query_rule_admits_a_percent_because_it_encodes_one`] for the other place this module
/// deliberately disagrees with `connector-spec`.
fn is_pin_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['{', '}', '"']) && !name.chars().any(char::is_whitespace)
}

/// Place every placeholder of a templated URL literal by where it falls in that URL.
///
/// The template is marked first — placeholders become opaque sentinels — so the authority is
/// delimited by the *emitter's* own `/`, `?` and `#` and never by a character a value could carry.
fn place_in_url(literal: &str, slots: &mut BTreeMap<String, Slot>) {
    let marked = scan_template(literal, |name| Some(mark(name)));
    let Some((_, after_scheme)) = marked.split_once("://") else {
        // Not a URL, so nothing here can say where these land. Left unplaced, which refuses more.
        return;
    };
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let query_start = after_scheme.find(['?', '#']).unwrap_or(after_scheme.len());
    for (offset, name) in marked_placeholders(after_scheme) {
        let slot = if offset < authority_end {
            Slot::Host
        } else if offset < query_start {
            Slot::Path
        } else {
            Slot::Query
        };
        record(slots, name, slot);
    }
}

/// Place each pin bind by where the symbol it binds is *used*.
fn place_pins(
    nodes: &[Node],
    pins: &BTreeMap<String, String>,
    literals: &BTreeMap<String, String>,
    slots: &mut BTreeMap<String, Slot>,
) {
    for node in nodes {
        match node {
            // `url = fmt("{base}/zones/{zone_id}?teamId={teamId}")`. Expanding the non-pin symbols
            // first is what makes the `?` scan read the whole URL: without it a `?` inside `base`
            // would be invisible and a query pin would be mistaken for a path segment.
            Node::Bind { value, .. } => match value.as_ref() {
                Node::Fmt { template } => {
                    let expanded = scan_template(template, |symbol| match pins.get(symbol) {
                        Some(variable) => Some(mark(variable)),
                        None => literals.get(symbol).cloned(),
                    });
                    for (offset, name) in marked_placeholders(&expanded) {
                        let slot = match expanded[..offset].contains(['?', '#']) {
                            true => Slot::Query,
                            false => Slot::Path,
                        };
                        record(slots, name, slot);
                    }
                }
                Node::Call { op, args } if op == HTTP_REQUEST => {
                    place_header_pins(args, pins, slots)
                }
                _ => {}
            },
            Node::Call { op, args } if op == HTTP_REQUEST => place_header_pins(args, pins, slots),
            Node::When {
                then, otherwise, ..
            } => {
                place_pins(then, pins, literals, slots);
                place_pins(otherwise, pins, literals, slots);
            }
            _ => {}
        }
    }
}

/// Any pin symbol used as a value in the `headers` record of an `http.request` call.
fn place_header_pins(
    args: &[Node],
    pins: &BTreeMap<String, String>,
    slots: &mut BTreeMap<String, Slot>,
) {
    let [Node::Obj { fields }] = args else {
        return;
    };
    let Some(headers) = fields.get("headers") else {
        return;
    };
    let Node::Obj { fields: headers } = headers.as_ref() else {
        return;
    };
    for value in headers.values() {
        if let Node::Var { name } = value.as_ref() {
            if let Some(variable) = pins.get(&name.0) {
                record(slots, variable, Slot::Header);
            }
        }
    }
}

/// Each `\u{1}name\u{1}` in `marked`, as `(byte offset of the marker, name)`.
fn marked_placeholders(marked: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut rest = marked;
    let mut consumed = 0;
    while let Some(open) = rest.find(MARK) {
        let after = &rest[open + MARK.len_utf8()..];
        let Some(close) = after.find(MARK) else {
            break;
        };
        found.push((consumed + open, &after[..close]));
        consumed += open + close + 2 * MARK.len_utf8();
        rest = &after[close + MARK.len_utf8()..];
    }
    found
}

/// Record `slot` for `variable`, collapsing to [`Slot::Unplaced`] if it disagrees with a placement
/// already made. One variable in two positions is not a shape the emitter produces; if it ever is,
/// the honest answer is the one that refuses the most rather than whichever was seen last.
fn record(slots: &mut BTreeMap<String, Slot>, variable: &str, slot: Slot) {
    match slots.get(variable) {
        Some(existing) if *existing == slot => {}
        Some(_) => {
            slots.insert(variable.to_owned(), Slot::Unplaced);
        }
        None => {
            slots.insert(variable.to_owned(), slot);
        }
    }
}

/// Build the request `declaration` makes when called with `params`, over `endpoints`.
///
/// `endpoints` maps each name [`endpoint_variables`] reported to the value the host's configuration
/// port supplied. It is **complete or the call does not get here** — [`crate::Operation`] resolves
/// every variable and refuses first — so substitution is total by construction rather than by
/// inspection.
///
/// # Errors
///
/// [`Error::MissingParameter`] when the caller omitted a parameter the operation declares,
/// [`Error::UnsafePathParameter`] when a caller string would escape the URL path segment the
/// emitted Flux places it in,
/// [`Error::Unbuildable`] when the body contains something this evaluator does not model or a
/// free-form body is not JSON, [`Error::UnsafeConfig`] when a configuration value would reshape the
/// request it is substituted into (C-214), and [`Error::UnresolvedEndpoint`] when a configuration
/// variable is still named in the finished URL — which only a caller-supplied value can cause. All
/// refuse; none repairs.
pub(crate) fn build(
    operation: &str,
    declaration: &CompositeOpDecl,
    params: &Value,
    endpoints: &BTreeMap<String, String>,
    slots: &BTreeMap<String, Slot>,
    caller_path_parameters: &BTreeSet<String>,
) -> Result<Request, Error> {
    // **The invariant, executed** (C-232). Checked before a single parameter is bound, because the
    // question it answers is whether this body's braces mean what this module thinks they mean —
    // and every step below assumes they do.
    refuse_unconfigurable(operation, declaration)?;

    let mut env = Env::new();

    // Every declared parameter must be supplied, exactly as flux's own composite dispatch requires
    // — `execute_composite_call` fails with "missing required param" and `composite_signature` puts
    // every parameter in `required_params`. An *optional* parameter is one a caller may pass `null`
    // for, not one they may omit: the emitted `when` guard turns null into "do not send this
    // filter", while an absent path parameter would quietly leave `{ticket_id}` in the URL.
    for param in &declaration.params {
        let name = param.name.0.as_str();
        let value = params
            .get(name)
            .ok_or_else(|| Error::MissingParameter {
                operation: operation.to_owned(),
                parameter: name.to_owned(),
            })?
            .clone();
        if caller_path_parameters.contains(name) {
            if let Value::String(text) = &value {
                Slot::Path
                    .validate(text)
                    .map_err(|reason| Error::UnsafePathParameter {
                        operation: operation.to_owned(),
                        parameter: name.to_owned(),
                        reason,
                    })?;
            }
        }
        env.insert(name.to_string(), value);
    }

    let cx = Build {
        operation,
        endpoints,
        slots,
    };
    let mut request = match cx.run(&declaration.body.body, &mut env)? {
        Some(request) => request,
        None => {
            return Err(Error::Unbuildable {
                operation: operation.to_owned(),
                message: format!("its body makes no `{HTTP_REQUEST}` call"),
            })
        }
    };

    // **The second lock on "total or refused".** Every configuration variable had a value, so no
    // literal can have carried one through — but a caller may have passed a *parameter* whose text
    // spells one, and interpolating that leaves the brace in the URL. Sending it would be a request
    // to a host, or a path, that names a variable nobody resolved.
    if let Some(variable) = endpoints
        .keys()
        .find(|variable| request.url.contains(&format!("{{{variable}}}")))
    {
        return Err(Error::UnresolvedEndpoint {
            operation: operation.to_owned(),
            variable: variable.clone(),
            url: request.url,
        });
    }

    identify(&mut request);
    Ok(request)
}

/// **Give the request this software's identity, unless the connector already stated one** (C-223).
///
/// # Why here, and not in the host or in each connector
///
/// This function is one line long and its position is the whole of the story's second acceptance,
/// so the reasoning is recorded where the code is. The full argument is
/// [docs/designs/host-identity.md](../../../docs/designs/host-identity.md).
///
/// **Not the host.** `connectors-api` constructs no request of its own — `AGENTS.md`'s ownership
/// table forbids it, and every route ends in `connector_pack`'s `pack` or `resolve`. Its only lever is the client it
/// builds, and `codewandler-flux-web` 0.41.0 exposes none: neither `Client::builder()` site in
/// `egress.rs` calls `ClientBuilder::user_agent` and `WebOptions` carries no field for one. Even
/// granting the upstream field, a header set on the *client* is invisible to [`crate::DryRunTransport`],
/// which is zero-sized and holds no client by construction — so the rehearsal would report a request
/// the host does not make, which is the one thing C-145 exists to prevent. That single fact decides
/// it independently of what upstream does.
///
/// **Not a C-55 constant header per connector.** It is what `providers/resend.toml` does today and
/// what C-52 contemplated for GitHub, and it is the option to argue against rather than inherit.
/// Three reasons: the default becomes *absence*, so the forty-sixth connector's omission is silent
/// and surfaces as a vendor `403` naming authorization; a TOML literal cannot carry the build's
/// version, so the value ships a bare product word that is wrong at the next release — Resend's
/// shipped `"flux-connectors"` is already exactly that; and it puts the *host's* identity into
/// *compiler* data, which the compiler has no business having an opinion about.
///
/// **So: request assembly, in the pack.** It is the one funnel every path already shares —
/// [`crate::Operation::build_request`], [`crate::Operation::build_authenticated_request`] through
/// it, [`crate::DryRunTransport::dry_run`] through it, and [`crate::Rehearsal`] through this same
/// `build`. Agreement between the rehearsal and the wire is therefore structural rather than a
/// property two code paths maintain in parallel.
///
/// # The connector still wins, and there is never a second header
///
/// The check is **case-insensitive**, which is not fastidiousness: `Request::headers` is a
/// `BTreeMap`, so a module setting `user-agent` and a default inserting `User-Agent` would be two
/// entries, two JSON keys, and — depending on how the transport folds them — two headers on the wire
/// or a silent overwrite. A duplicated `User-Agent` is its own defect and this is where it is
/// prevented. `providers/resend.toml` and any connector after it keep the value they declare.
///
/// If `codewandler-flux-web` later sets a client-level default, this stays correct and stays
/// necessary: reqwest treats a per-request header as an override of a client default, so there is no
/// duplication, and the dry run still needs its own answer.
fn identify(request: &mut Request) {
    if request
        .headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case(USER_AGENT))
    {
        return;
    }
    request
        .headers
        .insert(USER_AGENT.to_owned(), DEFAULT_USER_AGENT.to_owned());
}

/// The symbols an op body has bound so far.
type Env = BTreeMap<String, Value>;

/// One request being built: the operation it belongs to, and the configuration its literals are
/// substituted against.
///
/// A context rather than two more arguments on five functions, and it is where the operation id the
/// refusals quote lives.
struct Build<'a> {
    /// The operation id, for a refusal that names what it refused.
    operation: &'a str,
    /// The host's resolved endpoint values, keyed as [`endpoint_variables`] reports them.
    endpoints: &'a BTreeMap<String, String>,
    /// Where each of those values lands, as [`endpoint_slots`] read it off the same body. A
    /// variable absent from this map is [`Slot::Unplaced`] and is refused the most.
    slots: &'a BTreeMap<String, Slot>,
}

impl Build<'_> {
    /// This operation could not be built, and why.
    fn unbuildable(&self, message: String) -> Error {
        Error::Unbuildable {
            operation: self.operation.to_owned(),
            message,
        }
    }

    /// Run `nodes`, stopping at the `http.request` call.
    fn run(&self, nodes: &[Node], env: &mut Env) -> Result<Option<Request>, Error> {
        for node in nodes {
            match node {
                // `response = http.request(…)`: the request is the whole point of the body, so it
                // ends the walk. Nothing after it can change what is sent.
                Node::Bind { value, .. } if matches!(value.as_ref(), Node::Call { .. }) => {
                    let Node::Call { op, args } = value.as_ref() else {
                        unreachable!("the guard above matched a call");
                    };
                    return self.request_of(op, args, env).map(Some);
                }
                // A bare statement-position call. The emitter binds its response rather than
                // discarding it (C-9), so this is not a shape the catalogue carries — it is here
                // because reading the request off it is the same operation, and refusing it would
                // be a refusal of spelling rather than of substance.
                Node::Call { op, args } => return self.request_of(op, args, env).map(Some),
                Node::Bind { name, value, .. } => {
                    let value = self.eval(value, env)?;
                    env.insert(name.0.clone(), value);
                }
                Node::When {
                    cond,
                    then,
                    otherwise,
                } => {
                    let taken = if truthy(&self.eval(cond, env)?) {
                        then
                    } else {
                        otherwise
                    };
                    if let Some(request) = self.run(taken, env)? {
                        return Ok(Some(request));
                    }
                }
                // The emitted body ends `return $response`, which cannot be reached before the
                // request is built. Anything the walk has not modelled is refused rather than
                // skipped.
                Node::Return { .. } => break,
                other => {
                    return Err(self.unbuildable(format!(
                        "its body contains {}, which this pack does not evaluate",
                        kind(other)
                    )))
                }
            }
        }
        Ok(None)
    }

    /// Read `{ method, url, headers, body }` off the `http.request` call the body ends in.
    fn request_of(&self, op: &str, args: &[Node], env: &Env) -> Result<Request, Error> {
        if op != HTTP_REQUEST {
            return Err(self.unbuildable(format!(
                "its body calls `{op}`, and this pack delegates only `{HTTP_REQUEST}`"
            )));
        }
        let [Node::Obj { fields }] = args else {
            return Err(self.unbuildable(format!(
                "its `{HTTP_REQUEST}` call takes something other than one named record"
            )));
        };

        let mut request = Request {
            // `http.request` itself defaults an absent method to GET; the emitter always states one,
            // so this default is the same answer arrived at twice rather than a guess.
            method: "GET".to_string(),
            url: String::new(),
            headers: BTreeMap::new(),
            body: None,
        };
        for (name, value) in fields {
            match name.as_str() {
                "url" => request.url = text(&self.eval(value, env)?),
                "method" => request.method = text(&self.eval(value, env)?),
                "body" => request.body = Some(text(&self.eval(value, env)?)),
                "headers" => {
                    let Node::Obj { fields } = value.as_ref() else {
                        return Err(
                            self.unbuildable("its request headers are not a record".to_string())
                        );
                    };
                    for (header, value) in fields {
                        request
                            .headers
                            .insert(header.clone(), text(&self.eval(value, env)?));
                    }
                }
                other => {
                    return Err(self.unbuildable(format!(
                        "its request names `{other}`, which this pack does not carry"
                    )))
                }
            }
        }

        if request.url.is_empty() {
            return Err(self.unbuildable("its request has no URL".to_string()));
        }
        Ok(request)
    }

    /// Evaluate one pure value node.
    fn eval(&self, node: &Node, env: &Env) -> Result<Value, Error> {
        match node {
            // **The one substitution point.** A brace in a literal is a configuration variable and
            // never a symbol, because flux does not interpolate a `lit` — see this module's
            // documentation for why doing it here rather than over the finished URL is the half
            // that keeps caller data out of it.
            Node::Lit {
                value: Value::String(literal),
            } => Ok(Value::String(self.substitute(literal)?)),
            Node::Lit { value } => Ok(value.clone()),
            Node::Var { name } => env.get(&name.0).cloned().ok_or_else(|| {
                self.unbuildable(format!("its body reads `{}` before binding it", name.0))
            }),
            Node::Fmt { template } => Ok(Value::String(interpolate(template, env))),
            Node::Obj { fields } => {
                let mut object = serde_json::Map::new();
                for (name, value) in fields {
                    object.insert(name.clone(), self.eval(value, env)?);
                }
                Ok(Value::Object(object))
            }
            // `parse($body, as: "json")` is how a free-form body reaches the vendor at all: it
            // canonicalizes a record *and* validates a JSON string, so both spellings of "here is my
            // body" arrive intact. A string that is not JSON is refused here rather than sent.
            Node::Parse { value, as_type } if as_type == "json" => match self.eval(value, env)? {
                Value::String(text) => serde_json::from_str(&text).map_err(|source| {
                    self.unbuildable(format!(
                        "its body was supplied as text that is not JSON: {source}"
                    ))
                }),
                other => Ok(other),
            },
            other => Err(self.unbuildable(format!(
                "its body computes {}, which this pack does not evaluate",
                kind(other)
            ))),
        }
    }

    /// Fill this operation's configuration variables into a string literal — **checking each value
    /// against the position it lands on** (C-214).
    ///
    /// The same brace grammar [`interpolate`] uses, over the host's values instead of the body's
    /// symbols — one scanner, so the two cannot drift into disagreeing about what a placeholder is.
    ///
    /// This is the one place a configuration value and the request it shapes are both in hand, and
    /// that is why the guard lives here rather than in the loader: the loader validates a
    /// `ConfigField::example` and a pinned *name*, never the value a tenant supplies through a
    /// [`ConfigStore`](crate::ConfigStore).
    fn substitute(&self, literal: &str) -> Result<String, Error> {
        if !literal.contains('{') {
            return Ok(literal.to_owned());
        }
        // `scan_template` cannot carry a `Result` out of its callback, so the first refusal is
        // parked and returned before the filled string is used for anything. Filling continues with
        // the raw value purely so the scan finishes; that string is discarded.
        let mut refusal = None;
        let filled = scan_template(literal, |name| {
            let value = self.endpoints.get(name)?;
            Some(self.check(name, value).unwrap_or_else(|error| {
                refusal.get_or_insert(error);
                value.clone()
            }))
        });
        if let Some(error) = refusal {
            return Err(error);
        }
        self.check_authority(literal, &filled)?;
        Ok(filled)
    }

    /// One value, against the position it lands on. Returns the text to substitute — which differs
    /// from the value only for [`Slot::Query`], the one position with an encoder.
    fn check(&self, variable: &str, value: &str) -> Result<String, Error> {
        let slot = self.slots.get(variable).copied().unwrap_or(Slot::Unplaced);
        slot.validate(value).map_err(|reason| Error::UnsafeConfig {
            operation: self.operation.to_owned(),
            variable: variable.to_owned(),
            position: slot.word(),
            reason,
        })
    }

    /// **The host half**: the authority a templated URL composes must still be the authority the
    /// template declared.
    ///
    /// The check runs against the authority the *template* delimits — `literal` is marked first, so
    /// the span ends at an emitter-authored `/`, `?` or `#` and never at one a value carried in.
    /// The raw values are then substituted into that span and the result must be a hostname.
    ///
    /// That is what makes a value which *moves* the authority visible rather than invisible. With
    /// `subdomain = "acme.zendesk.com@evil.example"` the composed span is
    /// `acme.zendesk.com@evil.example.zendesk.com`, which is refused for the `@` — whereas reading
    /// the authority out of the *finished* URL would report `evil.example.zendesk.com`, a perfectly
    /// well-formed hostname, and say nothing at all.
    fn check_authority(&self, literal: &str, filled: &str) -> Result<(), Error> {
        let marked = scan_template(literal, |name| Some(mark(name)));
        let Some((_, after_scheme)) = marked.split_once("://") else {
            return Ok(());
        };
        let end = after_scheme
            .find(['/', '?', '#'])
            .unwrap_or(after_scheme.len());
        let template_authority = &after_scheme[..end];
        let hosted: Vec<&str> = marked_placeholders(template_authority)
            .into_iter()
            .map(|(_, name)| name)
            .filter(|name| self.endpoints.contains_key(*name))
            .collect();
        let Some(&first) = hosted.first() else {
            return Ok(());
        };

        let composed = fill_marked(template_authority, |name| self.endpoints.get(name).cloned());
        let refuse = |reason: String| Error::UnsafeConfig {
            operation: self.operation.to_owned(),
            variable: first.to_owned(),
            position: Slot::Host.word(),
            reason,
        };
        validate_templated_authority(template_authority, &composed).map_err(refuse)?;

        // A restatement of the property, executed rather than asserted in prose: no character the
        // rule above permits can delimit, so reading the finished URL the way a transport does must
        // yield exactly the string that was checked. If it ever does not, the span logic is wrong
        // and the request is refused rather than sent on a host nobody validated.
        let resolved = filled
            .split_once("://")
            .map(|(_, rest)| &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())]);
        if resolved != Some(composed.as_str()) {
            return Err(refuse(format!(
                "the composed host is {composed:?} but the URL resolves to \
                 {resolved:?}; the two must agree"
            )));
        }
        Ok(())
    }
}

/// Replace each `\u{1}name\u{1}` in `marked` with `fill(name)`, leaving an unfilled marker's name
/// verbatim.
///
/// The counterpart of [`marked_placeholders`], and the only place a configuration value enters a
/// marked string — after every marker has been located, so no value can be read as one.
fn fill_marked(marked: &str, mut fill: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(marked.len());
    let mut rest = marked;
    while let Some(open) = rest.find(MARK) {
        out.push_str(&rest[..open]);
        let after = &rest[open + MARK.len_utf8()..];
        let Some(close) = after.find(MARK) else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = &after[..close];
        out.push_str(&fill(name).unwrap_or_else(|| name.to_owned()));
        rest = &after[close + MARK.len_utf8()..];
    }
    out.push_str(rest);
    out
}

/// flux-lang's `interpolate_str`, over the symbols an op body has bound.
///
/// An unbound `{name}` is left verbatim, braces included — the behaviour the emitter's `when` guards
/// exist because of, so reproducing it is what keeps a guarded filter genuinely unsent rather than
/// sent as the literal text `{page}`.
fn interpolate(template: &str, env: &Env) -> String {
    scan_template(template, |name| env.get(name).map(text))
}

/// **The one brace grammar**, shared by all three things this module does with a template.
///
/// `fill` is called with each placeholder name in order; `Some` replaces it, `None` leaves it
/// verbatim — which is flux-lang's own `interpolate_str` behaviour and the whole reason the
/// emitter's `when` guards mean anything. [`interpolate`] fills from an op's bound symbols,
/// [`Build::substitute`] from the host's configuration, and [`scan_node`] answers `None` to every
/// name while recording it.
///
/// One scanner rather than three, because the alternative is three implementations of "what counts
/// as a placeholder" that agree until one of them does not — and the one that disagrees would be the
/// one deciding whether a URL still carries a variable.
fn scan_template(template: &str, mut fill: impl FnMut(&str) -> Option<String>) -> String {
    if !template.contains('{') {
        return template.to_string();
    }
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let at_brace = &rest[open..];
        let (open_token, close_token) = if at_brace.starts_with("{{") {
            ("{{", "}}")
        } else {
            ("{", "}")
        };
        let inner = &at_brace[open_token.len()..];
        let Some(close) = inner.find(close_token) else {
            out.push_str(at_brace);
            return out;
        };
        match fill(inner[..close].trim()) {
            Some(value) => {
                out.push_str(&value);
                rest = &inner[close + close_token.len()..];
            }
            None => {
                out.push_str(open_token);
                rest = inner;
            }
        }
    }
    out.push_str(rest);
    out
}

/// flux-lang's `lit_text`: a string is itself, `null` is empty, anything else is compact JSON.
///
/// The `null` arm is the one with consequences — see this module's documentation.
fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// flux-lang's `json_truthy`: null/false/0/empty are falsey, and so is the *text* `"false"`/`"0"`.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(bit) => *bit,
        Value::Number(number) => number.as_f64().map(|n| n != 0.0).unwrap_or(false),
        Value::String(text) => {
            let trimmed = text.trim();
            !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("false") && trimmed != "0"
        }
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
    }
}

/// A node's kind, for a refusal that names what it refused.
fn kind(node: &Node) -> &'static str {
    match node {
        Node::Call { .. } => "a call",
        Node::Bind { .. } => "a binding",
        Node::When { .. } => "a conditional",
        Node::Return { .. } => "a return",
        Node::Var { .. } => "a symbol",
        Node::Lit { .. } => "a literal",
        Node::Fmt { .. } => "an interpolation",
        Node::Obj { .. } => "a record",
        Node::List { .. } => "a list",
        Node::Parse { .. } => "a coercion",
        Node::Jq { .. } => "a path extraction",
        Node::Expr { .. } => "a formula",
        _ => "a statement",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// **Every configuration variable in the shipped catalogue is placed** (C-214) — **or is one of
    /// the few that are deliberately placed in more than one position** (C-229).
    ///
    /// [`Slot::Unplaced`] is the fail-closed default, and a fail-closed default that the shipped
    /// catalogue lands on *by accident* is not a default — it is the behaviour, and it would refuse
    /// values the vendor accepts. So the exemption is a **named list**, not a relaxation: a variable
    /// arriving here because the emitter grew a shape this module does not model is still red, and
    /// says which operation.
    ///
    /// `algolia/app_id` is the whole list today. Its field binds `endpoint.app_id` and also_binds
    /// `header.X-Algolia-Application-Id`, so one value composes the authority *and* travels as a
    /// header, and [`Slot::Unplaced`] is the intersection of the two rules rather than the absence
    /// of either — see that variant's own documentation for why that is the answer C-229 wanted.
    ///
    /// It also pins the placement itself. Getting `zone_id` wrong in the *lenient* direction —
    /// calling a path segment a query value — would encode a `/` instead of refusing it, which is
    /// the silent 404 this story argues against.
    #[test]
    fn every_shipped_configuration_variable_is_placed() {
        // Deliberately multiply placed, and therefore held to every position's rule at once.
        const EVERY_POSITION: &[&str] = &["algolia/app_id"];

        let mut unplaced = Vec::new();
        let mut multiple = std::collections::BTreeSet::new();
        let mut seen = std::collections::BTreeMap::new();
        for entry in catalog::operations() {
            let declaration = crate::spec::declaration_of(entry.id, entry.flux)
                .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
            let slots = endpoint_slots(&declaration);
            for variable in endpoint_variables(&declaration) {
                let named = format!("{}/{variable}", entry.provider);
                match slots.get(&variable) {
                    _ if EVERY_POSITION.contains(&named.as_str()) => {
                        assert_eq!(
                            slots.get(&variable),
                            Some(&Slot::Unplaced),
                            "`{}`: `{variable}` is listed as reaching every position, but this \
                             operation places it in exactly one — so either the list is stale or \
                             the field stopped binding two destinations",
                            entry.id
                        );
                        multiple.insert(named);
                    }
                    Some(Slot::Unplaced) | None => {
                        unplaced.push(format!("`{}`: `{variable}`", entry.id))
                    }
                    Some(slot) => {
                        seen.insert(named, *slot);
                    }
                }
            }
        }
        assert!(
            unplaced.is_empty(),
            "these shipped configuration variables could not be placed, so every value bound to \
             them is held to all three positions at once: {unplaced:#?}"
        );
        assert_eq!(
            multiple.iter().map(String::as_str).collect::<Vec<_>>(),
            EVERY_POSITION,
            "a variable listed as reaching every position no longer ships. Remove it here in the \
             same commit, so the exemption cannot outlive what earned it"
        );

        // The nine templated hosts, the path segments beside them, and C-187's two pins.
        assert_eq!(seen.get("zendesk/subdomain"), Some(&Slot::Host));
        assert_eq!(seen.get("shopify/shop"), Some(&Slot::Host));
        assert_eq!(seen.get("jira/site"), Some(&Slot::Host));
        assert_eq!(seen.get("okta/domain"), Some(&Slot::Host));
        assert_eq!(seen.get("freshdesk/domain"), Some(&Slot::Host));
        assert_eq!(seen.get("salesforce/instance"), Some(&Slot::Host));
        assert_eq!(seen.get("docusign/account_host"), Some(&Slot::Host));
        assert_eq!(seen.get("docusign/account_id"), Some(&Slot::Path));
        assert_eq!(seen.get("contentful/space_id"), Some(&Slot::Path));
        assert_eq!(seen.get("contentful/environment_id"), Some(&Slot::Path));
        assert_eq!(seen.get("statuspage/page_id"), Some(&Slot::Path));
        assert_eq!(seen.get("cloudflare/zone_id"), Some(&Slot::Path));
        assert_eq!(seen.get("vercel/teamId"), Some(&Slot::Query));
    }

    /// The measured host case, at the level of the rule rather than of a request.
    ///
    /// `evil.example.zendesk.com` is a perfectly good hostname, which is exactly why reading the
    /// authority off the *finished* URL cannot see anything wrong. What is refused is the string
    /// the **template** composes, and it carries the `@`.
    #[test]
    fn a_host_rule_refuses_what_moves_the_authority() {
        assert!(validate_authority("acme.zendesk.com").is_ok());
        assert!(validate_authority("acme-two.my.salesforce.com").is_ok());
        assert!(validate_authority("evil.example.zendesk.com").is_ok());

        for composed in [
            "acme.zendesk.com@evil.example.zendesk.com", // userinfo moves the origin
            "acme:8080.zendesk.com",                     // a port
            "acme/x.zendesk.com",                        // ends the authority early
            "acme%2e.zendesk.com",                       // a percent-escape that could decode
            "acme .zendesk.com",                         // whitespace
            "acme\n.zendesk.com",                        // a control character
            "acmé.zendesk.com",                          // non-ASCII, foldable by IDNA
            "..zendesk.com",                             // an empty label
            "{subdomain}.zendesk.com",                   // an unsubstituted placeholder
        ] {
            assert!(
                validate_authority(composed).is_err(),
                "{composed:?} was accepted as a host"
            );
        }
    }

    /// **What the host rule does *not* do, pinned so the claim cannot drift again.**
    ///
    /// The test above only ever composes `{label}.zendesk.com`, so everything it proves is about a
    /// value that must sit *inside* a suffix the template supplies. Four shipped connectors do not
    /// have that shape — `freshdesk` (`https://{domain}/api/v2`), `newrelic` (`https://{host}/v2`),
    /// `okta` (`https://{domain}/api/v1`) and `docusign` (`https://{account_host}/…`) template the
    /// **whole** authority. There the template contributes no fixed suffix, so "cannot introduce a
    /// delimiter" stops implying "cannot reach another host": the value *is* the host.
    ///
    /// That is the guard behaving as its own documentation says — it refuses delimiter injection,
    /// which is all it ever claimed — and it is why `crates/connectors-api/README.md` was wrong to
    /// summarise it as *"no shipped connector can be pointed at a loopback address"*. The layer that
    /// actually refuses loopback is flux's SSRF guard (`guard_url_scoped`, `PrivateNetAllow::None`),
    /// which the same section already cites one bullet earlier. Two layers were claimed; one exists.
    ///
    /// This test asserts the **current, correct** behaviour rather than a defect. If a future change
    /// makes a whole-host template require an operator-supplied allowlist, this test is the one that
    /// should fail and be rewritten deliberately.
    #[test]
    fn a_whole_host_template_is_constrained_only_to_being_a_hostname() {
        // The value is the entire authority for these four connectors, so any hostname composes.
        for whole_host in [
            "acme.freshdesk.com", // what an operator would really supply
            "evil.example.com",   // …and nothing in this rule prefers the first over this
            "127.0.0.1",          // refused later by the SSRF guard, NOT here
            "localhost",
            "169.254.169.254", // cloud metadata, likewise not this rule's job
        ] {
            assert!(
                validate_authority(whole_host).is_ok(),
                "{whole_host:?} was refused; if that is now intended, this test documents the \
                 boundary that moved and must be rewritten rather than deleted"
            );
        }

        // What the rule *does* still guarantee, even with no fixed suffix: the value cannot escape
        // the authority position. This half is load-bearing and must not regress.
        for escaping in [
            "evil.example.com/path",
            "evil.example.com:8080",
            "user@evil.example.com",
            "evil.example.com%2f",
        ] {
            assert!(
                validate_authority(escaping).is_err(),
                "{escaping:?} escapes the authority and must be refused"
            );
        }
    }

    #[test]
    fn a_literal_port_is_not_mistaken_for_a_configured_host_delimiter() {
        let template = format!("{}host{}:8089", MARK, MARK);
        assert!(validate_templated_authority(&template, "127.0.0.1:8089").is_ok());
        assert!(validate_templated_authority(&template, "pbx.internal:8089").is_ok());

        for escaped in [
            "pbx.internal:8088",
            "pbx.internal:8089@evil.example:8089",
            "pbx.internal:8089/path:8089",
        ] {
            assert!(
                validate_templated_authority(&template, escaped).is_err(),
                "{escaped:?} moved the declared authority"
            );
        }
    }

    /// The three request positions, and the one that encodes rather than refuses.
    #[test]
    fn each_position_answers_for_itself() {
        // A path segment refuses rather than encodes: a `zone_id` with a `/` in it is an operator's
        // mistake, and encoding it produces a 404 they cannot diagnose.
        assert_eq!(
            Slot::Path
                .validate("023e105f4ecef8ad9ca31a8372d0c353")
                .as_deref(),
            Ok("023e105f4ecef8ad9ca31a8372d0c353")
        );
        for bad in [
            "../../v4/other",
            "x/../../y",
            "abc?evil=1",
            "abc#frag",
            "abc%2Fdef",
            "a\nb",
            "..",
        ] {
            assert!(Slot::Path.validate(bad).is_err(), "{bad:?} was accepted");
        }

        // A query value refuses query *structure* and percent-encodes the rest. An ordinary team id
        // is unreserved throughout, so it travels unchanged — the shipped behaviour is untouched.
        assert_eq!(
            Slot::Query.validate("team_abc123").as_deref(),
            Ok("team_abc123")
        );
        assert_eq!(Slot::Query.validate("a/b:c").as_deref(), Ok("a%2Fb%3Ac"));
        for bad in ["team_a&projectId=evil", "a=b", "a?b", "a#b", "a b", "a\nb"] {
            assert!(Slot::Query.validate(bad).is_err(), "{bad:?} was accepted");
        }

        // A header value has no encoding at all, so a bad one can only be refused. Internal spaces
        // are legal and stay legal.
        assert_eq!(
            Slot::Header.validate("two words").as_deref(),
            Ok("two words")
        );
        for bad in ["a\r\nX-Evil: 1", "a\nb", " padded", "padded ", "café"] {
            assert!(Slot::Header.validate(bad).is_err(), "{bad:?} was accepted");
        }

        // Whitespace-only is not a value in any position, including the host.
        for slot in [
            Slot::Host,
            Slot::Path,
            Slot::Query,
            Slot::Header,
            Slot::Unplaced,
        ] {
            assert!(slot.validate(" ").is_err(), "{slot:?} accepted a space");
            assert!(slot.validate("\t").is_err(), "{slot:?} accepted a tab");
        }

        // Unplaced is held to every rule at once, so anything any position refuses it refuses.
        assert!(Slot::Unplaced.validate("plain").is_ok());
        for bad in ["a/b", "a&b", "a\nb"] {
            assert!(
                Slot::Unplaced.validate(bad).is_err(),
                "{bad:?} was accepted"
            );
        }
        // **Including the host rule**, which is the whole reason this arm is defence in depth. `@`
        // and `:` appear in none of the path, query or header charsets — no such position cares
        // about them — so without the host rule this arm accepted the exact value the story is
        // about, while its own comment claimed it was fail-closed.
        for bad in ["acme.zendesk.com@evil.example", "acme@evil", "acme:8080"] {
            assert!(
                Slot::Unplaced.validate(bad).is_err(),
                "{bad:?} was accepted by the fail-closed arm"
            );
            // The other direction, so the *reason* the host rule is needed here is executed rather
            // than asserted in a comment: all three of the other rules accept this value.
            assert!(
                validate_path(bad).is_ok()
                    && validate_query(bad).is_ok()
                    && validate_header(bad).is_ok(),
                "{bad:?} is refused by a non-host rule, so it proves nothing about the host rule"
            );
        }
    }

    /// **The one place the two spellings of the rule disagree**, pinned so the next person reads a
    /// measurement rather than a claim.
    ///
    /// `connector-spec`'s `Position::Query` refuses `%` (`crates/connector-spec/src/config.rs:303`,
    /// charset `&=?#+%`); [`validate_query`] does not (charset `&=?#+`). The loader's reason is that
    /// nothing percent-encodes a query value where *it* runs, which is true of a declared `example`
    /// and false here — [`crate::auth::query_encode`] maps `%` to `%25`, so the escape cannot
    /// survive to be re-read by the vendor.
    ///
    /// The asymmetry is fail-safe: the loader is stricter, so a provider author cannot ship an
    /// `example` a tenant would then be unable to supply. A drift the other way is the one that
    /// would hurt, and this test is where it would show up.
    #[test]
    fn the_query_rule_admits_a_percent_because_it_encodes_one() {
        for value in ["a%2Fb", "a%b", "100%", "50%off"] {
            assert_eq!(
                Slot::Query.validate(value).as_deref(),
                Ok(crate::auth::query_encode(value).as_str()),
                "{value:?} should be admitted and encoded, not refused"
            );
            let encoded = crate::auth::query_encode(value);
            assert!(
                !encoded.contains('%') || encoded.contains("%25"),
                "{value:?} encoded to {encoded:?}, leaving a `%` that was not itself escaped"
            );
        }
        // What the shared half still refuses, so this is a one-character divergence and not a
        // loosening of the query rule.
        for value in ["a&b", "a=b", "a?b", "a#b", "a+b", "a b", "a\nb"] {
            assert!(
                Slot::Query.validate(value).is_err(),
                "{value:?} was accepted"
            );
        }
    }

    fn env(pairs: &[(&str, Value)]) -> Env {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn an_unbound_placeholder_stays_verbatim() {
        assert_eq!(
            interpolate(
                "{base}/tickets/{id}.json",
                &env(&[("base", json!("https://x"))])
            ),
            "https://x/tickets/{id}.json"
        );
    }

    /// The `""`-is-absent idiom. `null` rendering as the text `null` would put `?page=null` on the
    /// wire in exactly the case a `when` guard exists to prevent.
    #[test]
    fn null_renders_as_nothing_rather_than_as_the_word() {
        assert_eq!(text(&Value::Null), "");
        assert_eq!(text(&json!(1)), "1");
        assert_eq!(text(&json!("x")), "x");
        assert_eq!(text(&json!({"a": 1})), r#"{"a":1}"#);
    }

    /// Truthiness is what makes an unsupplied filter unsent, so it must be flux's own rather than
    /// Rust's `Option`-shaped instinct.
    #[test]
    fn truthiness_is_fluxs_own() {
        assert!(!truthy(&Value::Null));
        assert!(!truthy(&json!("")));
        assert!(!truthy(&json!("false")));
        assert!(!truthy(&json!("0")));
        assert!(!truthy(&json!(0)));
        assert!(!truthy(&json!(false)));
        assert!(truthy(&json!("a")));
        assert!(truthy(&json!(1)));
    }

    /// **The plaintext does not print** (C-159, finding 1).
    ///
    /// A `Request` carries the assembled credential *after* [`crate::auth::place`] has run — in a
    /// header value, and for a query placement in the URL itself — and it is `pub`, so a host can
    /// hold one and format it. C-152 hand-wrote a redacting `Debug` for `auth::Assembled` for
    /// exactly this reason; the reviewer's observation was that this is the larger of the two
    /// exposures, because `Assembled` never escapes the crate and this does.
    ///
    /// What must still print is the shape of the call: the method, the host, the path, the header
    /// names and the query-parameter names. A `Debug` that printed nothing would be its own kind of
    /// defect.
    #[test]
    fn a_request_prints_its_shape_and_none_of_its_values() {
        const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-C159";

        let request = Request {
            method: "POST".to_string(),
            url: format!("https://vendor.example/api/v2/things?page=2&api_key={SENTINEL}"),
            headers: BTreeMap::from([
                ("Authorization".to_string(), format!("Bearer {SENTINEL}")),
                ("Content-Type".to_string(), "application/json".to_string()),
            ]),
            body: Some(format!(r#"{{"note":"{SENTINEL}"}}"#)),
        };

        let rendered = format!("{request:?}");
        assert!(
            !rendered.contains(SENTINEL),
            "the credential plaintext printed: {rendered}"
        );

        // And it is still worth reading: the method, the host, the path, the header names and the
        // query-parameter names all survive.
        assert!(rendered.contains("POST"), "{rendered}");
        assert!(rendered.contains("https://vendor.example"), "{rendered}");
        assert!(rendered.contains("/api/v2/things"), "{rendered}");
        assert!(rendered.contains("Authorization"), "{rendered}");
        assert!(rendered.contains("Content-Type"), "{rendered}");
        assert!(rendered.contains("api_key"), "{rendered}");
        assert!(rendered.contains("page"), "{rendered}");
    }

    /// A request with nothing to hide still says so, and an absent body reads as absent rather than
    /// as a redacted one — the distinction a reader of a `Debug` is actually chasing.
    #[test]
    fn a_request_with_no_query_and_no_body_prints_both_plainly() {
        let request = Request {
            method: "GET".to_string(),
            url: "https://vendor.example/api/v2/things".to_string(),
            headers: BTreeMap::new(),
            body: None,
        };

        let rendered = format!("{request:?}");
        assert!(
            rendered.contains("https://vendor.example/api/v2/things"),
            "{rendered}"
        );
        assert!(rendered.contains("body: None"), "{rendered}");
    }

    /// An empty header map and an absent body are omitted rather than sent as `{}` and `""`.
    #[test]
    fn a_bare_request_carries_only_what_it_has() {
        let request = Request {
            method: "GET".to_string(),
            url: "https://x/y".to_string(),
            headers: BTreeMap::new(),
            body: None,
        };
        assert_eq!(
            request.to_params(),
            json!({"url": "https://x/y", "method": "GET"})
        );
    }

    /// The emitted shape of a GraphQL operation: one endpoint, and the query document bound as a
    /// **string literal** because it is a vendor constant the caller may not choose (C-55's `const`
    /// body field). It is `linear-viewer` as C-110 actually emitted it, trimmed to its body.
    const GRAPHQL_OP: &str = r#"op probe-viewer -> Any
  description "Read the user this key belongs to"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """query Viewer {
  viewer {
    id
    name
    displayName
    email
    admin
  }
}
"""
  payload = { query }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
"#;

    /// **A GraphQL selection set is not configuration, and is no longer read as any** (C-232).
    ///
    /// [`endpoint_variables`] used to scan *every* string literal for `{…}`. That is sound for the
    /// two declared kinds — templated base URLs and C-187's pin binds — because in both a brace
    /// really does name a value a host supplies. A GraphQL query document is a third kind: it is a
    /// literal, it is full of braces, and **not one of them is configuration**. They are the
    /// vendor's own syntax, and the variable names the old scan produced were selection-set
    /// fragments, which made the resulting refusal unreadable as well as wrong.
    ///
    /// The document is now classified as unconfigurable and contributes no variable at all. The
    /// operation is refused instead — see the test below for the refusal, and this module's
    /// documentation for why refusing rather than ignoring is the honest answer.
    #[test]
    fn a_graphql_document_in_a_literal_is_not_read_as_configuration() {
        let declaration = crate::spec::declaration_of("probe-viewer", GRAPHQL_OP).expect("parses");

        assert_eq!(
            endpoint_variables(&declaration),
            Vec::<String>::new(),
            "a selection-set fragment was reported as a configuration variable a tenant could supply"
        );

        // And it is reported as the thing it is, so the refusal below has something to quote.
        let mut unclassified = Vec::new();
        walk_literals(&declaration.body.body, &mut |literal| {
            if let Some(literal) = unconfigurable(literal) {
                unclassified.push(literal.to_owned());
            }
        });
        assert_eq!(unclassified.len(), 1, "{unclassified:#?}");
        assert!(unclassified[0].contains("displayName"), "{unclassified:#?}");
    }

    /// **The production shape: an empty configuration, and the operation is refused** (C-232).
    ///
    /// This is the check whose absence let a fully green gate coexist with eight dead operations.
    /// `tests/request.rs::every_shipped_operation_builds_an_absolute_request` asserted only on the
    /// **URL**, and its `configuration()` helper manufactured a value for every *discovered*
    /// variable — so it fabricated exactly the values that hid this, and it would not have caught it
    /// for a connector whose documents live in the body.
    ///
    /// Three things are asserted together, because the second is worse than the first and the third
    /// is why the refusal has to come before either:
    ///
    /// 1. **Unconfigured, the operation refuses** — and it refuses for the *classification*, naming
    ///    the literal, rather than for a missing value named after a fragment of GraphQL.
    /// 2. **Configured, it still refuses.** Binding the fabricated values the old catalogue-wide
    ///    helper would have bound no longer buys a request: the refusal does not depend on what a
    ///    host was willing to answer, which is exactly the dependency C-232 is about.
    /// 3. **Substitution, if it were reached, would rewrite the document.** Kept as the record of
    ///    what the refusal prevents: `{ viewer { … } }` becomes the configuration value, so the
    ///    "constant query a caller must not choose" is chosen after all, by whoever supplies the
    ///    tenant's settings.
    ///
    /// Withdrawing the connector is what makes this a fixture rather than a shipped regression. The
    /// mechanism that would let a literal be opaque to this scan *and still ship* belongs to
    /// **C-87**, which publishes the configuration surface into the catalogue so the pack can
    /// **read** an operation's variables instead of inferring them from syntax. See
    /// `docs/designs/graphql-vendors.md`.
    #[test]
    fn a_graphql_operation_is_refused_rather_than_configured_or_corrupted() {
        let declaration = crate::spec::declaration_of("probe-viewer", GRAPHQL_OP).expect("parses");
        let slots = endpoint_slots(&declaration);

        // 1. The production shape: this connector declares no `[[config]]` field binding an
        //    endpoint, a path, a query or a header, so an operator supplies nothing.
        let refusal = build(
            "probe-viewer",
            &declaration,
            &json!({}),
            &BTreeMap::new(),
            &slots,
            &caller_path_parameters(&declaration),
        )
        .expect_err("a document literal is not configuration and cannot be built through");
        assert!(
            matches!(refusal, Error::Unbuildable { .. }),
            "refused, but for the wrong reason: {refusal}"
        );

        // 2. Configured with the fabricated values the old helper supplied — still refused.
        let fabricated: BTreeMap<String, String> = ["viewer", "document"]
            .iter()
            .map(|name| ((*name).to_string(), "a-value".to_string()))
            .collect();
        assert!(
            build(
                "probe-viewer",
                &declaration,
                &json!({}),
                &fabricated,
                &slots,
                &caller_path_parameters(&declaration),
            )
            .is_err(),
            "supplying values made the operation buildable again, which is the fabrication this \
             story exists to remove"
        );

        // 3. What the refusal prevents, measured at the substitution point itself so that removing
        //    the refusal cannot quietly remove the evidence for it too.
        let document = "query Viewer {\n  viewer {\n    displayName\n  }\n}\n";
        let endpoints: BTreeMap<String, String> = scan_names(document)
            .into_iter()
            .map(|name| (name, "a-value".to_string()))
            .collect();
        let corrupted = Build {
            operation: "probe-viewer",
            endpoints: &endpoints,
            slots: &BTreeMap::new(),
        }
        .substitute(document)
        .expect("substitution over a document has nothing to refuse it");
        assert!(
            !corrupted.contains("displayName"),
            "the document survived, so this no longer measures the corruption it exists for: \
             {corrupted}"
        );
        assert!(
            corrupted.contains("a-value"),
            "a configuration value did not reach the query document: {corrupted}"
        );
    }

    /// Every placeholder name in `template`, by the module's own brace grammar.
    fn scan_names(template: &str) -> Vec<String> {
        let mut names = Vec::new();
        scan_template(template, |name| {
            names.push(name.to_owned());
            None
        });
        names
    }

    /// **The "exactly two kinds" invariant, executed rather than asserted in prose** (C-232).
    ///
    /// This module's opening documentation used to state that the brace-carrying string literals in
    /// the shipped catalogue "are of exactly two kinds, and both are configuration", and then record
    /// in a caveat that C-110 had written a third. A statement about what ships is not a rule: the
    /// next connector with a brace in a literal falsifies it again, silently, because nothing
    /// executes it.
    ///
    /// So the two kinds are now the *only* two this module will read braces out of, and anything
    /// else is [`Error::Unbuildable`] rather than treated as configuration. That is the same choice
    /// the evaluator already makes for a node it does not model, applied to the scan: a literal
    /// whose braces are the vendor's own syntax is refused, not filled in with a tenant's settings.
    ///
    /// The fixture is C-110's `linear-viewer` exactly as it was emitted, which is the known
    /// positive — see `crates/connector-flux/tests/linear_connector.rs` for the provider file it
    /// came from.
    #[test]
    fn a_braced_literal_that_is_neither_a_url_nor_a_pin_is_refused() {
        let declaration = crate::spec::declaration_of("probe-viewer", GRAPHQL_OP).expect("parses");

        // The production shape: this connector declares no `[[config]]` field that binds an
        // endpoint, a path, a query or a header, so the configuration an operator supplies is
        // **empty**. Nothing may be fabricated to stand in for the document's braces.
        let error = build(
            "probe-viewer",
            &declaration,
            &json!({}),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &caller_path_parameters(&declaration),
        )
        .expect_err("a document literal is not configuration and cannot be built through");

        assert!(
            matches!(error, Error::Unbuildable { .. }),
            "refused, but for the wrong reason: {error}"
        );
        let message = error.to_string();
        assert!(
            message.contains("displayName"),
            "the refusal must quote the literal it could not classify, or an implementor cannot \
             find it: {message}"
        );
    }

    /// The two kinds themselves still classify, so the refusal above is not a refusal of everything.
    #[test]
    fn a_templated_url_and_a_pin_bind_are_the_two_kinds_that_do_classify() {
        for literal in [
            "https://{subdomain}.zendesk.com",
            "https://api.cloudflare.com/client/v4",
            "{zone_id}",
            "{ teamId }",
            "application/json",
        ] {
            assert!(
                unconfigurable(literal).is_none(),
                "{literal:?} is one of the two declared kinds and was refused"
            );
        }
        for literal in [
            "query Viewer {\n  viewer {\n    id\n  }\n}\n",
            "{\"already\": \"json\"}",
        ] {
            assert!(
                unconfigurable(literal).is_some(),
                "{literal:?} is neither a templated URL nor a pin bind and was accepted"
            );
        }
    }

    /// **Where this module's pin-name rule and the loader's disagree**, pinned so the next person
    /// reads a measurement rather than a claim — the same treatment
    /// [`the_query_rule_admits_a_percent_because_it_encodes_one`] gives the other divergence.
    ///
    /// `connector_spec::config`'s `parse_binding` requires a `path.`/`query.`/`header.` suffix to be
    /// **non-empty and nothing else**. [`is_pin_name`] adds exactly one clause: no whitespace and no
    /// `"`. The loader is authoritative for what a provider file may declare; this is where a pin
    /// stops being one this pack can *recognise in an emitted literal*, which is a different
    /// question and is answered more narrowly on purpose.
    #[test]
    fn the_pin_name_rule_is_narrower_than_the_loaders() {
        // Every one of these is a name the loader accepts, and the reconciliation ran far enough to
        // accept them here too. `page.size` is the measured case: `binds = "query.page.size"` loads
        // and emits `page_size = "{page.size}"`, and an identifier-only rule called that a document.
        for name in [
            "teamId",
            "zone_id",
            "page.size",
            "page[size]",
            "a-b",
            "_x",
            "9",
        ] {
            let literal = format!("{{{name}}}");
            assert_eq!(
                sole_placeholder(&literal),
                Some(name),
                "{literal:?} is a pin the loader accepts and this refused"
            );
            assert!(unconfigurable(&literal).is_none(), "{literal:?}");
        }

        // The one clause this module adds, and the shape it exists for: a JSON object literal
        // quotes its keys, so it can never be mistaken for a pin named after its own contents.
        for literal in ["{\"already\": \"json\"}", "{\"a\":1}", "{a b}", "{}", "{ }"] {
            assert!(
                sole_placeholder(literal).is_none(),
                "{literal:?} was read as a pin"
            );
        }

        // And the refusal says *which* clause, because "rename the field" and "rewrite the
        // operation" are opposite instructions.
        assert!(
            unconfigurable_reason("{my name}").contains("not a name this pack models"),
            "{}",
            unconfigurable_reason("{my name}")
        );
        assert!(
            unconfigurable_reason("query Viewer {\n  viewer {\n    id\n  }\n}\n")
                .contains("neither a templated URL nor a configuration pin"),
        );
    }
}
