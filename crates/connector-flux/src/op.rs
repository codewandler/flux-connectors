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
//! # What the IR cannot yet say about a request body
//!
//! Three shapes in `providers/*.toml` have no representation in
//! [`connector_spec::ParamSet`]`::body`, which is a flat `Vec<Param>` carrying one `name` each.
//! Where the gap is *detectable* this module refuses; where it is not, it is recorded here.
//!
//! 1. **Nested body paths — refused.** Zendesk's wire body is
//!    `{"ticket": {"comment": {"body": …}}}` and babelforce's agent-status update writes
//!    `presence.name`. A dotted body field name is refused ([`Error::NestedBodyField`]) rather than
//!    emitted as the literal key `"presence.name"`, which the vendor accepts and ignores. **This
//!    check is not complete**, and cannot be: `providers/zendesk.toml` records the caller-facing
//!    name in `name` and the wire path in the parameter's *description*, so those fields look flat
//!    and emit flat. Closing it needs an additive field on `Param` — a wire path such as
//!    `wire = "ticket.comment.body"`, which would serve the vendor-alias case (Freshdesk's
//!    `req_id` → `requester_id`) with the same field.
//! 2. **Constant body fields — handled.** Zendesk always sends `ticket.safe_update = true`, and
//!    `providers/zendesk.toml` pins it with a JSON Schema `const` for want of anywhere better.
//!    `const` already means "this value and no other", so [`constant`] reads it: the field is sent
//!    and does not become a parameter a model has to guess.
//! 3. **Free-form object bodies — undetectable.** `babelforce-call-session-set` and
//!    `babelforce-session-update` take `{"type": "object"}` bodies with no properties, so they
//!    declare no body parameters at all and emit a request with no body. `ParamSet` cannot say "the
//!    body is this one schema"; a `body_schema: Option<JsonSchema>` alongside `body` would.
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

use connector_spec::{Connector, HttpMethod, Idempotency, Operation, Param, Risk};
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
/// JSON request body, whose response is returned whole. Auth (C-10) and quirks compiled into
/// control flow (C-12) are omitted; the body shapes the IR cannot express are refused rather than
/// half-emitted. See the module documentation and [`Error`].
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

/// Lower one IR operation into the composite-op declaration flux-lang formats.
fn lower<'a>(connector: &Connector, operation: &'a Operation) -> Result<CompositeOpDecl> {
    // Checked before anything else: `format_composite_op` writes the name verbatim, so an
    // undeclarable id would produce text that does not parse rather than an error.
    if !flux_lang::ast::is_valid_decl_name(&operation.id) {
        return Err(Error::UnspellableOperationId {
            operation: operation.id.clone(),
        });
    }
    check_write_metadata(operation)?;

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

    for bound in &header {
        // `http.request` builds a `HeaderName` from this and errors on anything that is not an HTTP
        // token (`../flux/crates/flux-web/src/http.rs:172-174`). Catching it here turns a request
        // that could never be sent into a build failure.
        if !is_http_token(&bound.param.name) {
            return Err(Error::BadParamName {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
                reason: "an HTTP header name may contain only ASCII token characters",
            });
        }
    }
    for bound in &body {
        if bound.param.name.contains('.') {
            return Err(Error::NestedBodyField {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
            });
        }
    }

    // A constant body field is *sent* but never *declared*: pinning it with a JSON Schema `const`
    // already says "this value and no other", so asking a model to supply it would be asking it to
    // guess a value the schema fixes. See `constant`.
    let params: Vec<FluxParam> = path
        .iter()
        .chain(&query)
        .chain(&header)
        .chain(body.iter().filter(|b| constant(b.param).is_none()))
        .map(|b| FluxParam {
            name: SymbolName(b.symbol.clone()),
            ty: flux_type(&b.param.schema),
        })
        .collect();

    Ok(CompositeOpDecl {
        name: operation.id.clone(),
        params,
        returns: Some(TypeRef::Any),
        meta: metadata(operation)?,
        body: DraftAst {
            body: request_body(connector, operation, &path, &query, &header, &body)?,
            ..DraftAst::default()
        },
    })
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
    path: &[Bound<'_>],
    query: &[Bound<'_>],
    header: &[Bound<'_>],
    body_params: &[Bound<'_>],
) -> Result<Vec<Node>> {
    let (required, optional): (Vec<_>, Vec<_>) = query.iter().partition(|b| b.param.required);

    let mut template = String::from("{base}");
    template.push_str(&path_template(operation, path)?);
    for (i, bound) in required.iter().enumerate() {
        template.push(if i == 0 { '?' } else { '&' });
        template.push_str(&format!("{}={{{}}}", bound.param.name, bound.symbol));
    }

    let mut body = vec![
        // A literal today. When the endpoint moves into operator config (C-10) this one statement
        // is what changes; every URL downstream of it is already written against `{base}`.
        bind_string(BASE, connector.base_url.trim_end_matches('/')),
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
                    bound.param.name, bound.symbol
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

    // Headers: the media type the payload is encoded in, plus whatever the caller supplies. Auth
    // headers are deliberately absent — C-10 adds the credential reference.
    let mut headers: BTreeMap<String, Box<Node>> = BTreeMap::new();
    if !body_params.is_empty() {
        body.push(bind_string(CONTENT_TYPE, JSON_MEDIA_TYPE));
        headers.insert("content-type".to_string(), Box::new(symbol(CONTENT_TYPE)));
    }
    for bound in header {
        headers.insert(bound.param.name.clone(), Box::new(symbol(&bound.symbol)));
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
        let mut fields: BTreeMap<String, Box<Node>> = BTreeMap::new();
        for bound in body_params {
            if let Some(value) = constant(bound.param) {
                body.push(bind_lit(&bound.symbol, value.clone()));
            }
            fields.insert(bound.param.name.clone(), Box::new(symbol(&bound.symbol)));
        }
        body.push(Node::Bind {
            name: SymbolName(PAYLOAD.to_string()),
            value: Box::new(Node::Obj { fields }),
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
            .position(|b| b.param.name == wire)
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
fn from_tag<T: serde::de::DeserializeOwned>(tag: &'static str) -> Result<T> {
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
    use connector_spec::{ParamSet, Provenance};
    use serde_json::json;

    fn operation(path: &str, path_params: Vec<Param>) -> Operation {
        Operation {
            id: "vendor-thing-get".to_string(),
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
            description: String::new(),
            required: true,
            schema: json!({"type": "string"}),
        }
    }

    fn connector(base_url: &str, operation: Operation) -> Connector {
        Connector {
            id: "vendor".to_string(),
            vendor: String::new(),
            base_url: base_url.to_string(),
            description: String::new(),
            auth: Vec::new(),
            default_auth: Vec::new(),
            operations: vec![operation],
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

    /// A trailing slash on the connector's base URL must not become a double slash in the request.
    #[test]
    fn a_trailing_slash_on_the_base_url_is_trimmed() {
        let op = operation("/v2/agents", Vec::new());
        let emitted = emit_operation(&connector("https://api.example.com/", op.clone()), &op)
            .expect("a bare GET is inside the slice");
        assert!(
            emitted.contains(r#"$base = "https://api.example.com""#),
            "{emitted}"
        );
    }
}
