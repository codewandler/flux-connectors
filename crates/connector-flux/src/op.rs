//! One IR [`Operation`] → one formatted Flux `op` declaration.
//!
//! The whole module builds [`flux_lang`] AST nodes and hands them to flux-lang's own formatter. No
//! part of the generated text is assembled by this crate, which is what makes unparseable or
//! non-canonically-formatted output structurally impossible rather than merely unlikely
//! (AGENTS.md, *"Emit Flux through `flux_lang`, never through string templates"*).
//!
//! # The shape it emits
//!
//! ```flux
//! op freshdesk-ticket-note-add(id: Number, body: String, private: Bool) -> Any
//!   description "Add a note to a ticket; the note is private unless explicitly made public"
//!   risk "medium"
//!   idempotency "non_idempotent"
//!   effects ["network"]
//!   expose true
//!
//!   $base = "https://example.freshdesk.com/api/v2"
//!   $url = fmt("{base}/tickets/{id}/notes")
//!   $content_type = "application/json"
//!   $payload = { body: $body, private: $private }
//!   $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
//!   return $response
//! ```
//!
//! Seven decisions in that are worth stating, because later stories build on them.
//!
//! **`$base` is a seam, not decoration.** The connector's base URL is bound once and interpolated,
//! so C-10 has a single statement to replace when the endpoint starts coming from operator config
//! instead of from the IR. It is a literal today.
//!
//! **`risk` and `idempotency` come from the IR — and a write may not carry a read's.** flux's
//! approval gate reads them, and the IR makes both mandatory precisely so they cannot be decided by
//! silence. Nothing here defaults them; what this module adds is a *refusal*, in
//! [`check_write_metadata`], for the one direction that is unsafe — a state-changing method
//! declaring `risk = "low"` (which the gate waves through) or a `POST`/`PATCH` declaring itself
//! idempotent (which makes a `retry` around it unsound).
//!
//! **Required query parameters go in the URL template; optional ones are guarded.** An unbound
//! `{name}` placeholder is left *verbatim* in the string by flux's interpolator
//! (`interpolate_str`), so interpolating an unsupplied filter would send the vendor a literal
//! `?page={page}`. A `when $page` guard is what makes "not supplied" mean "not sent", and `$sep`
//! carries the `?`/`&` that only the first surviving parameter needs.
//!
//! **The body is a record bound to a symbol, never an inline one.** `http.request` reads its `body`
//! argument with `Value::as_str` (`../flux/crates/flux-web/src/http.rs:183-186`), so an inline
//! record reaches it as a JSON *object* and is dropped without a word. A bound record is stored as
//! canonical JSON text and arrives intact. Every literal the emitter contributes is bound too, for
//! a different reason — see [`bind_lit`].
//!
//! **The response is bound and returned.** C-8 ended in a bare `do http.request`, which discards
//! the statement result; the composite still yielded it by fall-through, which made the op's result
//! a property of the runtime rather than of the op. Nothing asserts on the status: a non-2xx is
//! *data* — `http.request` hands a 404 back with its status and succeeds — and turning that into an
//! op failure would throw away the response the caller needs to see.
//!
//! **No credential is emitted.** Auth is C-10 and is deliberately absent rather than stubbed: an
//! invented placeholder marker would be a second thing to migrate.
//!
//! # How a request body is shaped
//!
//! A vendor's body is rarely the flat map of a parameter list, and the three shapes that matter are
//! all expressible now (C-29). What each one costs is worth stating, because the failure mode they
//! share is that the *wrong* answer succeeds: a body a vendor does not recognize is not rejected,
//! it is accepted and ignored, and the caller sees a 200.
//!
//! 1. **Nested body paths — emitted.** Zendesk's wire body is
//!    `{"ticket": {"comment": {"body": …}}}` and babelforce's agent-status update writes
//!    `presence.name`. [`connector_spec::Param::wire`] carries the JSON path a body field occupies,
//!    and [`body_tree`] assembles those paths into one nested record. A body field whose *name* is
//!    dotted but which declares no `wire` is still refused ([`Error::NestedBodyField`]): whether it
//!    is a path or a literal key is exactly the thing that cannot be guessed.
//! 2. **Constant body fields — handled.** Zendesk always sends `ticket.safe_update = true`, and
//!    `providers/zendesk.toml` pins it with a JSON Schema `const` for want of anywhere better.
//!    `const` already means "this value and no other", so [`constant`] reads it: the field is sent
//!    and does not become a parameter a model has to guess.
//! 3. **Free-form object bodies — emitted through `parse`.** `babelforce-call-session-set` and
//!    `babelforce-session-update` take `{"type": "object"}` bodies with no properties, which
//!    [`connector_spec::ParamSet::body_schema`] declares. The caller supplies the whole body as one
//!    parameter — and it is **re-bound through `parse($body, as: "json")` rather than passed
//!    straight to `http.request`**. That is load-bearing: a composite op's parameter is stored with
//!    `Value::from_json` (flux-lang `runtime.rs:313-331`), so a caller-supplied record arrives as a
//!    `Value::Struct`, `http.request` reads its `body` argument with `Value::as_str`
//!    (`../flux/crates/flux-web/src/http.rs:182-186`), and the whole body is dropped without a
//!    word. `parse(…, as: "json")` canonicalizes a record *and* validates a JSON string
//!    (flux-lang `runtime.rs:4005-4010`), storing text either way — so both spellings of "here is
//!    my body" reach the vendor.
//!
//! # How a request header is decided
//!
//! Two kinds, and keeping them apart is the whole of C-55. A **caller-supplied** header is a
//! [`connector_spec::Param`] in `params.header` and becomes an argument, exactly as the IR
//! documents. A **vendor-fixed** header — `Accept: application/vnd.github+json`,
//! `Notion-Version: 2022-06-28`, an API version, a `User-Agent` — is a
//! [`connector_spec::ParamSet::const_headers`] entry: its value is bound as a literal and put in the
//! request record, and it never reaches the declared parameter list. A model is not asked to supply
//! a value the vendor has already decided, and no caller can overwrite one.
//!
//! `const` on a header *parameter* is [refused](Error::ConstantHeaderParam) rather than honoured. It
//! used to be a silent no-op — `constant` was consulted for `body` fields only, so the pin was
//! dropped and the parameter stayed — which shipped connectors whose mandatory header was whatever
//! the caller passed. Honouring it instead would make one list mean two things depending on a schema
//! keyword, and the IR would stop saying which headers a caller may set.
//!
//! One header name, one value: a name claimed twice — by a constant, by a caller-supplied parameter,
//! or by the `content-type` this emitter derives from the body — is
//! [refused](Error::HeaderConflict), because the request record has one slot per name and the second
//! claim would silently overwrite the first. **A constant header never carries a credential**; that
//! is enforced where the value is authored, in `connector_spec`'s loader, because only it can see
//! the credential and environment-variable names a value would have to name.
//!
//! # Two constraints on the *response* side, for the same reason
//!
//! `http.request` returns one flat string — `format!("HTTP {status}\n{headers}\n{body}")`
//! (`../flux/crates/flux-web/src/http.rs:221-225`) — so a declared error envelope cannot be dug out
//! in generated Flux. [`description`] states why in full and what is emitted instead.
//!
//! # Two constraints a reader will hit, stated here rather than discovered later
//!
//! **flux has no optional composite-op parameter.** Every declared param is required at call time:
//! `execute_composite_call` fails with *"missing required param"* when an argument is absent, and
//! `composite_signature` puts every param in `required_params` with `optional_params` left empty
//! (`../flux/crates/flux-flow/src/registry.rs:183-184`). So an IR parameter with
//! `required: false` still appears in the declaration — what optionality means here is that the
//! caller may pass **null**, and the `when` guard turns that into "do not send this filter". The
//! guard is truthiness, so a deliberate `0` or `false` is also treated as absent; no real filter in
//! the launch inventory is affected, but a future `?offset=0` would be.
//!
//! **Query values are interpolated verbatim — nothing percent-encodes them.** flux registers no
//! URL-encoding op (`../flux/crates/flux-flow/docs/ops-reference.md:15`), so a value carrying a
//! space, `&`, `#` or `=` corrupts the query string. Zendesk's search expressions are exactly this
//! shape, and its plugin percent-encodes them strictly for exactly this reason (inventory §3.3.5).
//! Half-encoding here would look correct and be wrong, so this emitter does not encode at all; the
//! fix belongs upstream in flux or in a quirk story, and is recorded rather than papered over.

use std::collections::BTreeMap;

use connector_spec::{Connector, HttpMethod, Idempotency, Operation, Param, Risk, FREE_FORM_BODY};
use flux_lang::ast::{DraftAst, Node, Param as FluxParam, SymbolName, TypeRef};
use flux_lang::program::{CompositeOpDecl, CompositeOpMeta};

use crate::names::Symbols;
use crate::types::flux_type;
use crate::{Error, Result};

/// The symbol holding the connector's base URL.
const BASE: &str = "base";
/// The symbol holding the request URL as it is assembled.
const URL: &str = "url";
/// The symbol holding the next query-string separator (`?` then `&`).
const SEP: &str = "sep";
/// The symbol holding the media type of the request body.
const CONTENT_TYPE: &str = "content_type";
/// The header that media type travels in — the one constant header the emitter owns, because it
/// describes a body only the emitter assembles.
const CONTENT_TYPE_HEADER: &str = "content-type";
/// The symbol holding the assembled JSON request body.
const PAYLOAD: &str = "payload";
/// The symbol holding the HTTP response.
const RESPONSE: &str = "response";
/// The one media type this emitter sends. Every launch provider is JSON over HTTP — Freshdesk sends
/// `content-type: application/json` on every request (inventory §4.3) and the other two agree.
const JSON_MEDIA_TYPE: &str = "application/json";

/// Emit `operation` as a formatted Flux `op` declaration, ready to concatenate into a module.
///
/// The returned text is canonical: it parses, and flux-lang's own formatter leaves it unchanged.
/// `tests/op_emitter.rs` asserts both rather than trusting them.
///
/// # Scope
///
/// An HTTP call whose parameters travel in the path, the query string, the request headers and a
/// JSON request body — nested at the JSON paths [`connector_spec::Param::wire`] declares, or
/// supplied whole by the caller — whose response is returned whole. Auth (C-10) and quirks compiled
/// into control flow (C-12) are omitted; a body declaration that could be read two ways is refused
/// rather than resolved by guesswork. See the module documentation and [`Error`].
pub fn emit_operation(connector: &Connector, operation: &Operation) -> Result<String> {
    Ok(flux_lang::format::format_composite_op(&lower(
        connector, operation,
    )?))
}

/// A parameter paired with the Flux symbol that carries it.
struct Bound<'a> {
    param: &'a Param,
    symbol: String,
}

/// The spelling the vendor sees: [`Param::wire`] when it is declared, the caller-facing name
/// otherwise. Every request position reads the name through this, so a declared alias cannot be
/// honored in one place and forgotten in another.
fn wire_name(param: &Param) -> &str {
    param.wire.as_deref().unwrap_or(&param.name)
}

/// A free-form body: the schema the caller's whole body must satisfy, and the symbol it arrives in.
struct FreeFormBody<'a> {
    schema: &'a connector_spec::JsonSchema,
    symbol: String,
}

/// A vendor-fixed header, and the symbol its literal value is bound to.
///
/// It carries a symbol for the same reason every other literal the emitter contributes does — see
/// [`bind_lit`] — and *not* because anything supplies it: no [`FluxParam`] is ever built from one.
struct ConstantHeader<'a> {
    /// The header name as the vendor sees it. Its own key is the wire name; there is no second
    /// spelling, because nobody calls it by a caller-facing one.
    name: &'a str,
    /// The value, emitted as a string literal.
    value: &'a str,
    symbol: String,
}

/// Every parameter of one operation, paired with the Flux symbol the emitted `op` declares for it.
struct Bindings<'a> {
    path: Vec<Bound<'a>>,
    query: Vec<Bound<'a>>,
    header: Vec<Bound<'a>>,
    body: Vec<Bound<'a>>,
    free_form: Option<FreeFormBody<'a>>,
    /// The headers the vendor fixes. Not parameters — see [`ConstantHeader`].
    const_headers: Vec<ConstantHeader<'a>>,
}

/// Allocate one Flux symbol per parameter, in the IR's own request-position order.
///
/// Extracted from [`lower`] because a **call site** needs the same answer: a graph node feeding an
/// operation (`graph.rs`) has to spell each argument with the symbol the operation's own declaration
/// uses, and a second copy of this order would drift the first time a request position moved.
fn bind_parameters<'a>(operation: &'a Operation) -> Result<Bindings<'a>> {
    // Path, query, header, body — the IR's own request-position order, so the declared parameter
    // list is stable across regeneration.
    let mut symbols = Symbols::new();
    let mut bind = |param: &'a Param| -> Result<Bound<'a>> {
        Ok(Bound {
            param,
            symbol: symbols.allocate(&operation.id, &param.name)?,
        })
    };
    let path: Vec<Bound<'_>> = operation
        .params
        .path
        .iter()
        .map(&mut bind)
        .collect::<Result<_>>()?;
    let query: Vec<Bound<'_>> = operation
        .params
        .query
        .iter()
        .map(&mut bind)
        .collect::<Result<_>>()?;
    let header: Vec<Bound<'_>> = operation
        .params
        .header
        .iter()
        .map(&mut bind)
        .collect::<Result<_>>()?;
    let body: Vec<Bound<'_>> = operation
        .params
        .body
        .iter()
        .map(&mut bind)
        .collect::<Result<_>>()?;

    // Two answers to one question. Merging them would need a rule nothing states — see
    // `Error::AmbiguousBody`.
    if operation.params.body_schema.is_some() && !body.is_empty() {
        return Err(Error::AmbiguousBody {
            operation: operation.id.clone(),
        });
    }
    let free_form = match &operation.params.body_schema {
        Some(schema) => Some(FreeFormBody {
            schema,
            symbol: symbols.allocate(&operation.id, FREE_FORM_BODY)?,
        }),
        None => None,
    };

    // Allocated last, from the same allocator, so a constant header can never take a symbol a
    // parameter would have had: adding one to a provider must not rename a symbol that already
    // travelled. They are not parameters and never reach the declared list.
    let const_headers: Vec<ConstantHeader<'_>> = operation
        .params
        .const_headers
        .iter()
        .map(|(name, value)| {
            Ok(ConstantHeader {
                name,
                value,
                symbol: symbols.allocate(&operation.id, name)?,
            })
        })
        .collect::<Result<_>>()?;

    Ok(Bindings {
        path,
        query,
        header,
        body,
        free_form,
        const_headers,
    })
}

/// The **caller-facing** name of every parameter a caller supplies, mapped to the Flux symbol the
/// emitted `op` declares for it.
///
/// A constant body field is deliberately absent: it is sent but never declared (see this module's
/// `constant`), so naming it at a call site would pass an argument the operation does not have.
///
/// **Public because it is the seam between the two schemas one operation now has.**
/// [`connector_spec::Operation::input_schema`] composes the catalogue's answer and keys it by the
/// caller-facing name; anything reading the *emitted declaration* — `connector-pack`'s `ToolSpec`
/// projection above all — sees the Flux symbol instead, because that is what a composite op can
/// declare. This map is that correspondence, computed once here where the allocation happens, so
/// the relationship between the two is mechanical rather than a coincidence two crates maintain
/// separately. `tests/input_schema_agreement.rs` holds them together over every shipped operation.
pub fn parameter_symbols(operation: &Operation) -> Result<BTreeMap<String, String>> {
    let bound = bind_parameters(operation)?;
    Ok(bound
        .path
        .iter()
        .chain(&bound.query)
        .chain(&bound.header)
        .chain(bound.body.iter().filter(|b| constant(b.param).is_none()))
        .map(|b| (b.param.name.clone(), b.symbol.clone()))
        .chain(
            bound
                .free_form
                .iter()
                .map(|free| (FREE_FORM_BODY.to_string(), free.symbol.clone())),
        )
        .collect())
}

/// Lower one IR operation into the composite-op declaration flux-lang formats.
fn lower(connector: &Connector, operation: &Operation) -> Result<CompositeOpDecl> {
    // Checked before anything else: `format_composite_op` writes the name verbatim, so an
    // undeclarable id would produce text that does not parse rather than an error.
    if !flux_lang::ast::is_valid_decl_name(&operation.id) {
        return Err(Error::UnspellableOperationId {
            operation: operation.id.clone(),
        });
    }
    check_write_metadata(operation)?;

    // Kept whole rather than destructured: `request_body` needs every group, and passing them one
    // slice at a time is how a lowering grows an argument list nobody can read.
    let bound = bind_parameters(operation)?;
    let Bindings {
        path,
        query,
        header,
        body,
        free_form,
        const_headers,
    } = &bound;

    for bound in header {
        // `const` on a header parameter used to be a silent no-op: the pin was dropped and the
        // parameter stayed. Refused rather than honoured — see `Error::ConstantHeaderParam`.
        if constant(bound.param).is_some() {
            return Err(Error::ConstantHeaderParam {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
            });
        }
        // `http.request` builds a `HeaderName` from this and errors on anything that is not an HTTP
        // token (`../flux/crates/flux-web/src/http.rs:172-174`). Catching it here turns a request
        // that could never be sent into a build failure. The check is on the *wire* name, which is
        // the one that reaches `HeaderName`.
        if !is_http_token(wire_name(bound.param)) {
            return Err(Error::BadParamName {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
                reason: "an HTTP header name may contain only ASCII token characters",
            });
        }
    }
    for bound in body {
        // A dotted name with no `wire` cannot be read either way — see `Error::NestedBodyField`.
        if bound.param.wire.is_none() && bound.param.name.contains('.') {
            return Err(Error::NestedBodyField {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
            });
        }
    }
    check_header_names(
        operation,
        header,
        const_headers,
        !body.is_empty() || free_form.is_some(),
    )?;

    // A constant body field is *sent* but never *declared*: pinning it with a JSON Schema `const`
    // already says "this value and no other", so asking a model to supply it would be asking it to
    // guess a value the schema fixes. See `constant`. A constant *header* is absent for the same
    // reason and needs no filter: it is not a parameter at all.
    let params: Vec<FluxParam> = path
        .iter()
        .chain(query)
        .chain(header)
        .chain(body.iter().filter(|b| constant(b.param).is_none()))
        .map(|b| FluxParam {
            name: SymbolName(b.symbol.clone()),
            ty: flux_type(&b.param.schema),
        })
        .chain(free_form.iter().map(|free| FluxParam {
            name: SymbolName(free.symbol.clone()),
            ty: flux_type(free.schema),
        }))
        .collect();

    Ok(CompositeOpDecl {
        name: operation.id.clone(),
        params,
        returns: Some(TypeRef::Any),
        meta: metadata(operation)?,
        body: DraftAst {
            body: request_body(connector, operation, &bound)?,
            ..DraftAst::default()
        },
    })
}

/// Every header the request will carry, checked once: spellable, and claimed by one declaration.
///
/// The record `http.request` receives has one slot per name, so a second claim on a name overwrites
/// the first — silently, in an order nothing in the provider file makes visible. That is why this is
/// a refusal rather than a merge, and it is the header-side twin of [`Error::BodyPathConflict`].
///
/// Three sources can claim a name: the media type the emitter derives from the request body, a
/// caller-supplied `params.header`, and a `const_headers` entry. The comparison is case-insensitive
/// because HTTP field names are (RFC 9110 §5.1) — a map keyed by spelling would hold `Notion-Version`
/// and `notion-version` happily, and send the header twice.
fn check_header_names(
    operation: &Operation,
    header: &[Bound<'_>],
    const_headers: &[ConstantHeader<'_>],
    has_body: bool,
) -> Result<()> {
    let mut claimed: Vec<(String, &'static str)> = Vec::new();
    let mut claim = |name: &str, source: &'static str| -> Result<()> {
        let folded = name.to_ascii_lowercase();
        if let Some((_, first)) = claimed.iter().find(|(seen, _)| *seen == folded) {
            return Err(Error::HeaderConflict {
                operation: operation.id.clone(),
                name: name.to_string(),
                first,
                second: source,
            });
        }
        claimed.push((folded, source));
        Ok(())
    };

    if has_body {
        claim(CONTENT_TYPE_HEADER, "the media type of the request body")?;
    }
    for bound in header {
        claim(wire_name(bound.param), "a caller-supplied parameter")?;
    }
    for constant_header in const_headers {
        if !is_http_token(constant_header.name) {
            return Err(Error::BadHeaderName {
                operation: operation.id.clone(),
                name: constant_header.name.to_string(),
            });
        }
        claim(constant_header.name, "a constant header")?;
    }
    Ok(())
}

/// The value a body field is pinned to, when its schema pins one.
///
/// JSON Schema's `const` means "this value and no other", which is exactly the property
/// `providers/zendesk.toml` needs for `ticket.safe_update`: always sent, never caller-supplied. The
/// IR has no dedicated field for it, so `const` is read as the declaration it already is rather
/// than a second one being invented.
fn constant(param: &Param) -> Option<&serde_json::Value> {
    param.schema.get("const")
}

/// Whether `name` is a valid HTTP field name (RFC 9110 §5.1 `token`).
fn is_http_token(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Whether the method changes state the vendor owns.
fn mutates(method: HttpMethod) -> bool {
    match method {
        HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete => true,
        HttpMethod::Get | HttpMethod::Head | HttpMethod::Options => false,
    }
}

/// **A write may not carry a read's metadata.** flux's approval gate reads `risk` and
/// `idempotency`, so a `POST` that inherited a `GET`'s `low`/`idempotent` would be auto-approved and
/// treated as safe to retry. Both are refused rather than corrected: a silent correction hides the
/// authoring mistake, and the IR omits `Default` on both enums for exactly that reason.
fn check_write_metadata(operation: &Operation) -> Result<()> {
    if !mutates(operation.method) {
        return Ok(());
    }
    if operation.risk == Risk::Low {
        return Err(Error::WriteDeclaredLowRisk {
            operation: operation.id.clone(),
            method: method_word(operation.method),
        });
    }
    // `PUT` and `DELETE` *are* idempotent methods under RFC 9110 §9.2.2, so declaring them so is
    // honest; `POST` and `PATCH` are not, and claiming otherwise makes a `retry` around the call
    // unsound.
    if operation.idempotency == Idempotency::Idempotent
        && matches!(operation.method, HttpMethod::Post | HttpMethod::Patch)
    {
        return Err(Error::WriteDeclaredIdempotent {
            operation: operation.id.clone(),
            method: method_word(operation.method),
        });
    }
    Ok(())
}

/// The `description`/`risk`/`idempotency`/`effects`/`expose` block — the `ToolSpec` surface flux
/// exposes to a model, and the fields its approval gate reads.
fn metadata(operation: &Operation) -> Result<CompositeOpMeta> {
    Ok(CompositeOpMeta {
        description: description(operation),
        risk: from_tag(risk_tag(operation.risk))?,
        idempotency: from_tag(idempotency_tag(operation.idempotency))?,
        // Every generated op makes an HTTP request; nothing else it does is an effect flux tracks.
        effects: vec![from_tag("network")?],
        // `expose true` is what surfaces the op to the model as an LLM tool.
        expose: true,
        ..CompositeOpMeta::default()
    })
}

/// The operation's description, extended with the vendor's error envelope when it declares one.
///
/// **Why the envelope lands in prose rather than in emitted control flow.** `http.request` returns
/// one flat string — `format!("HTTP {status}\n{headers}\n{body}")`
/// (`../flux/crates/flux-web/src/http.rs:221-225`) — not a record with a `body` field. Flux's `jq`
/// parses a *whole* string as JSON before extracting (`jq_parse_input`, flux-lang
/// `runtime.rs:4018`), so a pointer applied to that string resolves to `null` on every response,
/// success or failure: emitting one would look like envelope handling and never do any. Splitting
/// the body back out needs `split($response, '\n\n')`, and an escape inside an `expr` formula is not
/// a fixed point of flux's own formatter — the emitted module stops round-tripping.
///
/// So the response is returned whole, with the envelope's location stated on the contract the model
/// actually reads. Digging the message out mechanically needs `http.request` to return a *record*;
/// that is a seam story on flux, filed rather than faked.
fn description(operation: &Operation) -> String {
    let mut out = operation.description.clone();
    let Some(envelope) = &operation.quirks.error_envelope else {
        return out;
    };
    if !out.is_empty() && !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(
        "A non-2xx response is returned as data, not a failure: the vendor's error message is at `",
    );
    out.push_str(&envelope.message_pointer);
    out.push('`');
    if let Some(code) = &envelope.code_pointer {
        out.push_str(", its error code at `");
        out.push_str(code);
        out.push('`');
    }
    out.push_str(" in the response body.");
    out
}

/// The op body: bind the base URL, assemble the request URL and payload, issue the request, return
/// what came back.
fn request_body(
    connector: &Connector,
    operation: &Operation,
    bound: &Bindings<'_>,
) -> Result<Vec<Node>> {
    let Bindings {
        path,
        query,
        header,
        body: body_params,
        free_form,
        const_headers,
    } = bound;
    let free_form = free_form.as_ref();

    let (required, optional): (Vec<_>, Vec<_>) = query.iter().partition(|b| b.param.required);

    let mut template = String::from("{base}");
    template.push_str(&path_template(operation, path)?);
    for (i, bound) in required.iter().enumerate() {
        template.push(if i == 0 { '?' } else { '&' });
        template.push_str(&format!("{}={{{}}}", wire_name(bound.param), bound.symbol));
    }

    let mut body = vec![
        // A literal today. When the endpoint moves into operator config (C-10) this one statement
        // is what changes; every URL downstream of it is already written against `{base}`.
        //
        // **The operation's own service's base URL**, not the connector's (C-49). For a
        // `default`-only provider the two are the same string, which is why every shipped module was
        // byte-identical when services landed; for a multi-service one they differ — Google serves
        // Gmail from `gmail.googleapis.com` and Calendar from `www.googleapis.com` — and binding the
        // connector's would send the request to a host that does not serve it, while the manifest
        // that installs alongside this module named the right one. The two artifacts of one service
        // must not disagree about where the traffic goes, and the manifest already resolves through
        // `base_url_of`.
        bind_string(
            BASE,
            connector
                .base_url_of(&operation.service)
                .trim_end_matches('/'),
        ),
        bind_fmt(URL, template),
    ];

    if !optional.is_empty() {
        // The first *surviving* optional parameter opens the query string — unless a required one
        // already did.
        body.push(bind_string(
            SEP,
            if required.is_empty() { "?" } else { "&" },
        ));
        for (i, bound) in optional.iter().enumerate() {
            let mut guarded = vec![bind_fmt(
                URL,
                format!(
                    "{{{URL}}}{{{SEP}}}{}={{{}}}",
                    wire_name(bound.param),
                    bound.symbol
                ),
            )];
            // The last parameter never needs to hand a separator on.
            if i + 1 < optional.len() {
                guarded.push(bind_string(SEP, "&"));
            }
            body.push(Node::When {
                cond: Box::new(Node::Var {
                    name: SymbolName(bound.symbol.clone()),
                }),
                then: guarded,
                otherwise: Vec::new(),
            });
        }
    }

    let mut request = BTreeMap::from([
        ("url".to_string(), Box::new(symbol(URL))),
        (
            "method".to_string(),
            Box::new(Node::Lit {
                value: serde_json::Value::String(method_word(operation.method).to_string()),
            }),
        ),
    ]);

    // Headers: the media type the payload is encoded in, the ones the vendor fixes, plus whatever
    // the caller supplies. Auth headers are deliberately absent — C-10 adds the credential
    // reference, and a constant header may not carry one (`connector_spec`'s loader refuses it).
    let has_body = !body_params.is_empty() || free_form.is_some();
    let mut headers: BTreeMap<String, Box<Node>> = BTreeMap::new();
    if has_body {
        body.push(bind_string(CONTENT_TYPE, JSON_MEDIA_TYPE));
        headers.insert(
            CONTENT_TYPE_HEADER.to_string(),
            Box::new(symbol(CONTENT_TYPE)),
        );
    }
    for bound in header {
        headers.insert(
            wire_name(bound.param).to_string(),
            Box::new(symbol(&bound.symbol)),
        );
    }
    // A vendor-fixed header is *sent* and never *declared*: the value is the vendor's, so asking a
    // caller for it would be asking for a value that has exactly one right answer — the same
    // reasoning a constant body field already carries. Bound to a symbol first, like every literal
    // this emitter contributes to a record; see `bind_lit`.
    for constant_header in const_headers {
        body.push(bind_string(&constant_header.symbol, constant_header.value));
        headers.insert(
            constant_header.name.to_string(),
            Box::new(symbol(&constant_header.symbol)),
        );
    }
    if !headers.is_empty() {
        request.insert(
            "headers".to_string(),
            Box::new(Node::Obj { fields: headers }),
        );
    }

    // The JSON body, assembled as a record and passed **by symbol**. That is load-bearing rather
    // than stylistic: `http.request` reads its `body` argument with `Value::as_str`
    // (`../flux/crates/flux-web/src/http.rs:183-186`), so an inline record arrives as a JSON object
    // and is silently dropped, whereas a bound record is stored as canonical JSON *text* and
    // arrives intact.
    if !body_params.is_empty() {
        for bound in body_params {
            if let Some(value) = constant(bound.param) {
                body.push(bind_lit(&bound.symbol, value.clone()));
            }
        }
        body.push(Node::Bind {
            name: SymbolName(PAYLOAD.to_string()),
            value: Box::new(body_tree(operation, body_params)?.into_node()),
            ty: None,
            effect: None,
        });
        request.insert("body".to_string(), Box::new(symbol(PAYLOAD)));
    } else if let Some(free) = free_form {
        // Re-bound rather than passed straight through, and that is the whole point: a parameter
        // holding a record is stored as a `Value::Struct` (flux-lang `runtime.rs:313-331`) and
        // `http.request` reads `body` with `Value::as_str`, so `body: $body` would send *no body at
        // all*. `parse(…, as: "json")` canonicalizes a record and validates a JSON string, storing
        // text either way — see the module documentation.
        body.push(Node::Bind {
            name: SymbolName(PAYLOAD.to_string()),
            value: Box::new(Node::Parse {
                value: Box::new(symbol(&free.symbol)),
                as_type: "json".to_string(),
            }),
            ty: None,
            effect: None,
        });
        request.insert("body".to_string(), Box::new(symbol(PAYLOAD)));
    }

    // Bound and returned, not discarded. `do http.request` throws the statement result away; the
    // composite still yields it by fall-through, but what the op returns is then a property of the
    // runtime rather than of the op. C-8 left this explicit `return` to C-9.
    body.push(Node::Bind {
        name: SymbolName(RESPONSE.to_string()),
        value: Box::new(Node::Call {
            op: "http.request".to_string(),
            args: vec![Node::Obj { fields: request }],
        }),
        ty: None,
        effect: None,
    });
    // Nothing asserts on the status. A non-2xx is data — `http.request` hands a 404 back with its
    // status and succeeds (`../flux/crates/flux-web/src/http.rs:219-225`) — and a caller that can
    // read the status is strictly better served than one whose flow aborted.
    body.push(Node::Return {
        value: Box::new(symbol(RESPONSE)),
    });

    Ok(body)
}

/// The request body under construction: a JSON object tree, keyed by wire path segment.
///
/// A tree rather than a flat map because a vendor's body nests — Zendesk's comment text lives at
/// `ticket.comment.body` — and the flat spelling `{"ticket.comment.body": …}` is a request Zendesk
/// accepts and ignores. `BTreeMap` at every level is what makes the emitted record deterministic
/// without sorting anything at emit time.
#[derive(Debug)]
enum BodyNode {
    /// A caller-supplied (or constant) value, carried by this Flux symbol.
    Leaf(String),
    /// An object of further paths.
    Branch(BTreeMap<String, BodyNode>),
}

impl BodyNode {
    /// Place `symbol_name` at `segments`.
    ///
    /// `Err(depth)` reports that the first `depth` segments of the path are contested — either they
    /// already hold a value this field would have to live *inside*, or another field already claimed
    /// exactly them. The caller turns that into [`Error::BodyPathConflict`]; reporting the depth
    /// rather than a message is what lets one recursive step stay ignorant of the whole path.
    fn insert(&mut self, segments: &[&str], symbol_name: &str) -> std::result::Result<(), usize> {
        let BodyNode::Branch(children) = self else {
            // This node already holds a value, so the path that reached it cannot also be an
            // object. The caller adds the depth it was reached at.
            return Err(0);
        };
        let (head, rest) = segments.split_first().expect("a wire path has a segment");
        if rest.is_empty() {
            if children.contains_key(*head) {
                return Err(1);
            }
            children.insert((*head).to_string(), BodyNode::Leaf(symbol_name.to_string()));
            return Ok(());
        }
        children
            .entry((*head).to_string())
            .or_insert_with(|| BodyNode::Branch(BTreeMap::new()))
            .insert(rest, symbol_name)
            .map_err(|depth| depth + 1)
    }

    /// Lower the tree into the nested Flux record the payload is bound to.
    fn into_node(self) -> Node {
        match self {
            BodyNode::Leaf(symbol_name) => symbol(&symbol_name),
            BodyNode::Branch(children) => Node::Obj {
                fields: children
                    .into_iter()
                    .map(|(key, child)| (key, Box::new(child.into_node())))
                    .collect(),
            },
        }
    }
}

/// Assemble every body field into one tree, each at the JSON path its [`Param::wire`] names.
///
/// Both ways a path set can be incoherent are refused rather than resolved, because either
/// resolution silently drops a field the author declared: an empty segment
/// ([`Error::BadWirePath`]), and a path that needs to be both a value and an object
/// ([`Error::BodyPathConflict`]).
fn body_tree(operation: &Operation, body_params: &[Bound<'_>]) -> Result<BodyNode> {
    let mut root = BodyNode::Branch(BTreeMap::new());
    // Every path placed so far, so a conflict can name *both* fields rather than only the one that
    // arrived second.
    let mut placed: Vec<(&str, &str)> = Vec::new();

    for bound in body_params {
        let wire = wire_name(bound.param);
        let segments: Vec<&str> = wire.split('.').collect();
        if segments.iter().any(|segment| segment.is_empty()) {
            return Err(Error::BadWirePath {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
                wire: wire.to_string(),
            });
        }

        match root.insert(&segments, &bound.symbol) {
            Ok(()) => placed.push((wire, bound.param.name.as_str())),
            Err(depth) => {
                let path = segments[..depth].join(".");
                // The field already occupying that path is the one whose own path is it, or lies
                // under it.
                let first = placed
                    .iter()
                    .find(|(placed_wire, _)| {
                        *placed_wire == path || placed_wire.starts_with(&format!("{path}."))
                    })
                    .map(|(_, name)| *name)
                    .unwrap_or(bound.param.name.as_str());
                return Err(Error::BodyPathConflict {
                    operation: operation.id.clone(),
                    first: first.to_string(),
                    second: bound.param.name.clone(),
                    path,
                });
            }
        }
    }

    Ok(root)
}

/// The vendor path template with each `{wire_name}` rewritten to `{symbol_name}`, so the `fmt`
/// interpolation resolves against the symbols the op actually declares.
///
/// Both directions of mismatch are refused: a placeholder with no declared parameter would
/// interpolate to a literal `{name}` in the URL, and a declared path parameter that never appears
/// in the template could never travel.
fn path_template(operation: &Operation, path: &[Bound<'_>]) -> Result<String> {
    let mut out = String::new();
    let mut used = vec![false; path.len()];
    let mut rest = operation.path.as_str();

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            return Err(Error::UndeclaredPathParam {
                operation: operation.id.clone(),
                path: operation.path.clone(),
                name: after.to_string(),
            });
        };
        let wire = &after[..close];
        let index = path
            .iter()
            .position(|b| wire_name(b.param) == wire)
            .ok_or_else(|| Error::UndeclaredPathParam {
                operation: operation.id.clone(),
                path: operation.path.clone(),
                name: wire.to_string(),
            })?;
        used[index] = true;
        out.push_str(&format!("{{{}}}", path[index].symbol));
        rest = &after[close + 1..];
    }
    out.push_str(rest);

    if let Some(index) = used.iter().position(|u| !u) {
        return Err(Error::UnusedPathParam {
            operation: operation.id.clone(),
            path: operation.path.clone(),
            name: path[index].param.name.clone(),
        });
    }

    // A base URL is joined to the path verbatim, so the path has to carry its own leading slash.
    if !out.starts_with('/') {
        out.insert(0, '/');
    }
    Ok(out)
}

/// `$name` in expression position.
fn symbol(name: &str) -> Node {
    Node::Var {
        name: SymbolName(name.to_string()),
    }
}

/// `$name = "<literal>"`.
fn bind_string(name: &str, value: &str) -> Node {
    bind_lit(name, serde_json::Value::String(value.to_string()))
}

/// `$name = <json literal>`.
///
/// Every literal the emitter contributes to a record is bound to a symbol first, and that is a
/// requirement rather than a preference: a record whose values are *all* literals is not "dynamic"
/// to flux-lang's formatter, which then spells it as an `@json` blob that flux's own CST formatter
/// re-spaces — so the emitted module stops being a fixed point of the formatter and
/// `emitted_text_is_a_fixed_point_of_the_flux_formatter` fails. Binding keeps every record the
/// emitter builds spellable in native Flux.
fn bind_lit(name: &str, value: serde_json::Value) -> Node {
    Node::Bind {
        name: SymbolName(name.to_string()),
        value: Box::new(Node::Lit { value }),
        ty: None,
        effect: None,
    }
}

/// `$name = fmt("<template>")`.
fn bind_fmt(name: &str, template: String) -> Node {
    Node::Bind {
        name: SymbolName(name.to_string()),
        value: Box::new(Node::Fmt { template }),
        ty: None,
        effect: None,
    }
}

/// flux-lang does not re-export `flux_spec`'s `Risk`, `Idempotency` or `Effect`, and this crate
/// must not take a direct dependency on `flux-spec` to name them — the flux-lang pin is the only
/// coupling to flux this repo has, and widening it is a reviewed change (AGENTS.md).
///
/// All three deserialize from their stable snake_case tags, so the metadata block is built by
/// reading those tags back through serde. The *values* still come from the IR; only the spelling
/// travels as text, and `metadata_tags_are_the_ones_flux_lang_accepts` pins every tag this crate
/// can produce, so the error below is unreachable in practice rather than merely unlikely.
pub(crate) fn from_tag<T: serde::de::DeserializeOwned>(tag: &'static str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(tag.to_string()))
        .map_err(|source| Error::UnknownMetadataTag { tag, source })
}

/// The IR's risk vocabulary is flux's own (`flux_spec::Risk`), so this is a rename, not a policy.
fn risk_tag(risk: Risk) -> &'static str {
    match risk {
        Risk::Low => "low",
        Risk::Medium => "medium",
        Risk::High => "high",
        Risk::Destructive => "destructive",
    }
}

/// Likewise for idempotency (`flux_spec::Idempotency`).
fn idempotency_tag(idempotency: Idempotency) -> &'static str {
    match idempotency {
        Idempotency::Idempotent => "idempotent",
        Idempotency::NonIdempotent => "non_idempotent",
        Idempotency::Conditional => "conditional",
    }
}

/// The method as `http.request` spells it.
fn method_word(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Head => "HEAD",
        HttpMethod::Options => "OPTIONS",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use connector_spec::{ParamSet, Provenance, DEFAULT_SERVICE};
    use serde_json::json;

    fn operation(path: &str, path_params: Vec<Param>) -> Operation {
        Operation {
            id: "vendor-thing-get".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Get,
            path: path.to_string(),
            description: "Get a thing.".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            auth: None,
            params: ParamSet {
                path: path_params,
                ..ParamSet::default()
            },
            response_schema: None,
            quirks: Default::default(),
        }
    }

    fn path_param(name: &str) -> Param {
        Param {
            name: name.to_string(),
            wire: None,
            description: String::new(),
            required: true,
            schema: json!({"type": "string"}),
        }
    }

    fn body_param(name: &str, wire: Option<&str>) -> Param {
        Param {
            name: name.to_string(),
            wire: wire.map(str::to_string),
            description: String::new(),
            required: true,
            schema: json!({"type": "string"}),
        }
    }

    /// One `Bound` per body parameter, with the symbol the allocator would hand out.
    fn bind_all<'a>(operation: &'a Operation, params: &'a [Param]) -> Vec<Bound<'a>> {
        let mut symbols = Symbols::new();
        params
            .iter()
            .map(|param| Bound {
                param,
                symbol: symbols
                    .allocate(&operation.id, &param.name)
                    .expect("a test parameter name is spellable"),
            })
            .collect()
    }

    fn connector(base_url: &str, operation: Operation) -> Connector {
        Connector {
            id: "vendor".to_string(),
            authority: None,
            api_version: None,
            services: Vec::new(),
            vendor: String::new(),
            base_url: base_url.to_string(),
            description: String::new(),
            auth: Vec::new(),
            default_auth: Vec::new(),
            operations: vec![operation],
            events: Vec::new(),
            channels: Vec::new(),
            config: Vec::new(),
            verify: None,
            graphs: Vec::new(),
            provenance: Provenance::default(),
        }
    }

    /// Every tag this crate can produce is one flux-lang reads back. This is what makes
    /// [`from_tag`]'s error unreachable rather than merely unlikely, so the exhaustive `match`es
    /// above are the only place the vocabulary is stated.
    #[test]
    fn metadata_tags_are_the_ones_flux_lang_accepts() {
        for risk in [Risk::Low, Risk::Medium, Risk::High, Risk::Destructive] {
            let mut op = operation("/a", Vec::new());
            op.risk = risk;
            metadata(&op).unwrap_or_else(|e| panic!("risk {risk:?} must encode: {e}"));
        }
        for idempotency in [
            Idempotency::Idempotent,
            Idempotency::NonIdempotent,
            Idempotency::Conditional,
        ] {
            let mut op = operation("/a", Vec::new());
            op.idempotency = idempotency;
            metadata(&op)
                .unwrap_or_else(|e| panic!("idempotency {idempotency:?} must encode: {e}"));
        }
    }

    #[test]
    fn a_path_placeholder_is_rewritten_to_its_symbol() {
        let op = operation("/v2/calls/{call.id}", vec![path_param("call.id")]);
        let bound = vec![Bound {
            param: &op.params.path[0],
            symbol: "call_id".to_string(),
        }];
        assert_eq!(path_template(&op, &bound).unwrap(), "/v2/calls/{call_id}");
    }

    #[test]
    fn a_missing_leading_slash_is_supplied() {
        let op = operation("v2/agents", Vec::new());
        assert_eq!(path_template(&op, &[]).unwrap(), "/v2/agents");
    }

    /// A placeholder nothing declares would interpolate to a literal `{id}` in the request URL.
    #[test]
    fn an_undeclared_path_placeholder_is_refused() {
        let op = operation("/v2/calls/{id}", Vec::new());
        assert!(matches!(
            path_template(&op, &[]),
            Err(Error::UndeclaredPathParam { .. })
        ));
    }

    /// The other direction: a declared path parameter with nowhere to go.
    #[test]
    fn a_path_parameter_that_never_appears_is_refused() {
        let op = operation("/v2/calls", vec![path_param("id")]);
        let bound = vec![Bound {
            param: &op.params.path[0],
            symbol: "id".to_string(),
        }];
        assert!(matches!(
            path_template(&op, &bound),
            Err(Error::UnusedPathParam { .. })
        ));
    }

    /// The tree is what turns a set of wire paths into one nested record. Asserted on the AST
    /// rather than on emitted text, so a formatting change cannot make this pass or fail.
    #[test]
    fn wire_paths_assemble_into_one_nested_record() {
        let op = operation("/a", Vec::new());
        let params = vec![
            body_param("body", Some("ticket.comment.body")),
            body_param("public", Some("ticket.comment.public")),
            body_param("updated_stamp", Some("ticket.updated_stamp")),
        ];
        let bound = bind_all(&op, &params);
        let record = body_tree(&op, &bound)
            .expect("a coherent path set")
            .into_node();

        let Node::Obj { fields: root } = &record else {
            panic!("a body is a record");
        };
        assert_eq!(root.keys().collect::<Vec<_>>(), ["ticket"]);
        let Node::Obj { fields: ticket } = root["ticket"].as_ref() else {
            panic!("`ticket` holds an object");
        };
        assert_eq!(
            ticket.keys().collect::<Vec<_>>(),
            ["comment", "updated_stamp"]
        );
        let Node::Obj { fields: comment } = ticket["comment"].as_ref() else {
            panic!("`ticket.comment` holds an object");
        };
        assert_eq!(comment.keys().collect::<Vec<_>>(), ["body", "public"]);
        assert!(matches!(
            comment["body"].as_ref(),
            Node::Var { name } if name.0 == "body"
        ));
    }

    /// A field with no `wire` keeps sitting at the root of the body — the existing encoding, and the
    /// one every provider that does not nest still relies on.
    #[test]
    fn a_field_without_a_wire_path_stays_at_the_root() {
        let op = operation("/a", Vec::new());
        let params = vec![body_param("subject", None)];
        let bound = bind_all(&op, &params);
        let BodyNode::Branch(children) = body_tree(&op, &bound).expect("a flat body") else {
            panic!("the root of a body is an object");
        };
        assert!(children.contains_key("subject"), "{:?}", children.keys());
    }

    /// `ticket.comment` and `ticket.comment.body` cannot both exist: `comment` would have to be a
    /// value and an object at once, so one field would be dropped from the request.
    #[test]
    fn two_fields_that_need_one_path_to_be_two_things_are_refused() {
        let op = operation("/a", Vec::new());
        for order in [
            vec![
                body_param("comment", Some("ticket.comment")),
                body_param("body", Some("ticket.comment.body")),
            ],
            vec![
                body_param("body", Some("ticket.comment.body")),
                body_param("comment", Some("ticket.comment")),
            ],
        ] {
            let bound = bind_all(&op, &order);
            let error = body_tree(&op, &bound).expect_err("a contested path is not emittable");
            assert!(
                matches!(&error, Error::BodyPathConflict { path, .. } if path == "ticket.comment"),
                "the refusal must name the contested path, got: {error}"
            );
            // Both sides are named, so an author does not have to find the other one.
            let rendered = error.to_string();
            assert!(
                rendered.contains("comment") && rendered.contains("body"),
                "{rendered}"
            );
        }
    }

    /// Two fields claiming exactly the same path is the same failure without the nesting.
    #[test]
    fn two_fields_claiming_one_path_are_refused() {
        let op = operation("/a", Vec::new());
        let params = vec![
            body_param("stamp", Some("ticket.updated_stamp")),
            body_param("updated_stamp", Some("ticket.updated_stamp")),
        ];
        let bound = bind_all(&op, &params);
        assert!(matches!(
            body_tree(&op, &bound),
            Err(Error::BodyPathConflict { .. })
        ));
    }

    /// An empty segment is not a JSON key any vendor means, and left alone it would emit
    /// `{"a": {"": …}}` — accepted and ignored, like every other body mistake here.
    #[test]
    fn an_empty_wire_path_segment_is_refused() {
        let op = operation("/a", Vec::new());
        for wire in ["", ".a", "a.", "a..b"] {
            let params = vec![body_param("field", Some(wire))];
            let bound = bind_all(&op, &params);
            assert!(
                matches!(body_tree(&op, &bound), Err(Error::BadWirePath { .. })),
                "`{wire}` must be refused"
            );
        }
    }

    /// The loader refuses this in an authored file; the emitter refuses it again, because
    /// `http.request` builds a `HeaderName` from the name it is handed and an IR does not have to
    /// have come from a provider TOML — spec ingest (C-4) produces one too.
    #[test]
    fn a_constant_header_name_that_is_not_a_token_is_refused() {
        let mut op = operation("/v2/agents", Vec::new());
        op.params.const_headers = BTreeMap::from([("X Api Version".to_string(), "2".to_string())]);
        let connector = connector("https://api.example.com", op.clone());
        assert!(matches!(
            emit_operation(&connector, &op),
            Err(Error::BadHeaderName { .. })
        ));
    }

    /// `content-type` describes a body only this emitter assembles, so a constant claiming the name
    /// is refused rather than merged: the request record has one slot for it, and whichever
    /// declaration lost would be dropped without a word.
    #[test]
    fn a_constant_header_may_not_claim_the_media_type() {
        let mut op = operation("/v2/agents", Vec::new());
        op.method = HttpMethod::Post;
        op.risk = Risk::Medium;
        op.idempotency = Idempotency::NonIdempotent;
        op.params.body = vec![body_param("subject", None)];
        op.params.const_headers = BTreeMap::from([(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )]);
        let connector = connector("https://api.example.com", op.clone());
        assert!(matches!(
            emit_operation(&connector, &op),
            Err(Error::HeaderConflict { .. })
        ));
    }

    /// A trailing slash on the connector's base URL must not become a double slash in the request.
    #[test]
    fn a_trailing_slash_on_the_base_url_is_trimmed() {
        let op = operation("/v2/agents", Vec::new());
        let emitted = emit_operation(&connector("https://api.example.com/", op.clone()), &op)
            .expect("a bare GET is inside the slice");
        assert!(
            emitted.contains(r#"base = "https://api.example.com""#),
            "{emitted}"
        );
    }

    /// **A service's own base URL is what its operations request** (C-49).
    ///
    /// The manifest a service installs with already carries `base_url_of(service)`, so an op body that
    /// bound the *connector's* value would make the two halves of one installable unit disagree: the
    /// module would call a host the manifest does not list, which is also the host C-10's `http_hosts`
    /// will be derived from. Google is the shipped case — Gmail on `gmail.googleapis.com`, Calendar on
    /// `www.googleapis.com` — and the override is honored here, at the emitter, rather than by every
    /// provider having to repeat its host per operation.
    #[test]
    fn an_operations_service_base_url_overrides_the_connectors() {
        let mut op = operation("/gmail/v1/users/me/labels", Vec::new());
        op.service = "gmail".to_string();

        let mut connector = connector("https://www.googleapis.com", op.clone());
        connector.services = vec![connector_spec::Service {
            name: "gmail".to_string(),
            description: String::new(),
            base_url: Some("https://gmail.googleapis.com/".to_string()),
            api_version: Some("v1".to_string()),
            roles: Vec::new(),
        }];

        let emitted = emit_operation(&connector, &op).expect("a bare GET is inside the slice");
        assert!(
            emitted.contains(r#"base = "https://gmail.googleapis.com""#),
            "the op must request its own service's host, trailing slash trimmed:\n{emitted}"
        );
        assert!(
            !emitted.contains("www.googleapis.com"),
            "the connector's default host must not survive the override:\n{emitted}"
        );
    }
}
