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
//! **Query parameters are structured data, never URL text — C-30.** Flux 0.54's
//! `http.request(query: ...)` owns RFC 3986 encoding and omits null values, so optional `false` and
//! `0` remain real values instead of disappearing behind a truthiness guard. Arrays, objects and
//! `Any` are refused because the IR does not yet declare a vendor's collection convention.
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
//! 3. **Arrays in the body — emitted, at a declared length (C-185).** A `wire` segment may carry a
//!    bracketed index, and `personalizations[0].to[0].email` puts a caller's value inside two
//!    nested arrays of objects. That is SendGrid's envelope, and before this an envelope-shaped
//!    vendor could not be addressed at all: every segment was an object key, so the body came out
//!    as `{"personalizations": {"0": …}}`, which is a 400 rather than a shorthand.
//!
//!    **What it solves is the fixed-length envelope, not the caller-supplied list**, and the two
//!    are different problems. The indices come from the provider file, so the array's length is a
//!    property of the declaration: a one-recipient send is one `[0]`, a two-recipient send is a
//!    second set of fields. A *batch* — the caller passes n items and n elements are built — would
//!    need the emitter to compute over caller data, which is the expression language this
//!    repository refuses (AGENTS.md, *Flow graph contract*). The caller can still supply a whole
//!    array as one value (`providers/notion.toml`'s rich-text title), and that is the older bargain
//!    unchanged: the caller assembles the shape and nothing here checks it. An index may also
//!    address a whole *element* — `properties.title[0]` — which splits the difference: this
//!    repository supplies the array wrapper, the caller supplies the element.
//!
//!    **What it refuses**, each because the alternative is a request the vendor answers:
//!    a hole in the indices ([`Error::SparseBodyArray`]); a bare numeric segment, which used to
//!    build an object keyed `"0"` and now sits one character from a spelling that means something
//!    else ([`Error::NumericWirePathSegment`]); anything else brackets can spell, including an
//!    array directly inside an array ([`Error::BadArrayIndex`]); a body whose *root* is an array,
//!    which needs an empty first segment and is refused as one ([`Error::BadWirePath`]) — every
//!    root-array vendor this repository has looked at wants a caller-supplied batch, so a
//!    fixed-length spelling would buy nothing; and an indexed path under `form` encoding, which
//!    C-144 refuses nesting for already.
//!
//!    **How deep it goes: as deep as an author spells, and no deeper.** There is no recursion and
//!    no untyped blob, because every leaf of the tree is a declared parameter — the body is always
//!    the finite set of fields the provider file lists, which is the property C-107 and C-168 were
//!    protecting when they refused a recursive body model.
//!
//!    **An optional field inside an element behaves exactly as it does inside an object**, which is
//!    to say C-56's problem is unchanged and unsolved here: it is sent as an explicit `null` rather
//!    than omitted. An *element* cannot be optional at all — the indices are declared, so a
//!    two-element envelope always sends two elements.
//! 4. **Free-form object bodies — emitted through `parse`.** `babelforce-call-session-set` and
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
//! # How the body is *encoded* — orthogonal to all three
//!
//! [`connector_spec::ParamSet::body_encoding`] says whether the assembled body is a JSON document or
//! `application/x-www-form-urlencoded` pairs, and the `content-type` header is **derived from it**
//! rather than authored (C-144). `json` is the default and the emitter's output for an operation that
//! declares nothing is byte-for-byte what it was before the axis existed. A `form` body is assembled
//! as text by [`form_payload`], whose documentation states what that costs; the shapes form encoding
//! cannot express are refused by [`check_body_encoding`] rather than flattened.
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
//! caller may pass **null**, and Flux omits that field from the structured query map.

use std::collections::BTreeMap;

use connector_spec::{
    Connector, HttpMethod, Idempotency, Operation, Param, Position, Risk, FREE_FORM_BODY,
};
use flux_lang::ast::{DraftAst, Node, Param as FluxParam, SymbolName, TypeRef};
use flux_lang::program::{CompositeOpDecl, CompositeOpMeta};

use crate::names::Symbols;
use crate::types::flux_type;
use crate::{Error, Result};

/// The symbol holding the connector's base URL.
const BASE: &str = "base";
/// The symbol holding the request URL as it is assembled.
const URL: &str = "url";
/// The symbol holding the media type of the request body.
const CONTENT_TYPE: &str = "content_type";
/// The header that media type travels in — the one constant header the emitter owns, because it
/// describes a body only the emitter assembles.
const CONTENT_TYPE_HEADER: &str = "content-type";
/// The symbol holding the assembled request body.
const PAYLOAD: &str = "payload";
/// The symbol holding the next separator between form pairs, for a form body whose every field is
/// optional. The first *surviving* pair must not be preceded by an `&`.
const FORM_SEP: &str = "form_sep";
/// The symbol holding the HTTP response.
const RESPONSE: &str = "response";

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
    let printed = flux_lang::format::format_composite_op(&lower(connector, operation)?);
    canonical(operation, printed)
}

/// Re-print the AST printer's text through flux's **CST** formatter, and emit that — C-185.
///
/// flux-lang has two formatters and they do not always agree. `flux_lang::format` prints an AST;
/// `flux_lang::format_cst::format_module` re-prints parsed *text* and is the one a human editing a
/// generated file runs, so it is the one the C-11 canonicality gate compares against
/// (`tests/op_emitter.rs`, every per-provider `…_emits_an_analyzable_module`). Measured on
/// flux-lang 0.49.0: a list expression is `[ a, b ]` from the AST printer and `[a, b]` from the CST
/// formatter, while a record is `{ a: b }` from both. So the moment a body carried an array, the
/// emitter's own documented promise — *"flux-lang's own formatter leaves it unchanged"* — stopped
/// being true, and it stopped being true for the whole module rather than for the array.
///
/// Taking the CST spelling is the fix that does not involve this crate touching generated text. It
/// is safe by construction rather than by inspection: `format_module` carries an **equivalence
/// guard** (`format_cst.rs:51-61`) and returns `Some` only for text it has re-parsed and lowered to
/// the identical module, comments included — so this can change spacing and never meaning, and its
/// failure mode is `None`, which is a refusal. That is the same reasoning
/// [`Error::UnspellableDuration`] already records for the other place the two formatters disagree,
/// and the same conclusion, arrived at from the other side: a duration has no CST spelling at all,
/// so it is refused; a list has one, so it is used.
///
/// **It is inert on everything already shipped.** Every committed module is already a fixed point of
/// `format_module` — that is what the canonicality tests assert — so re-printing one returns it
/// unchanged. `cargo run -p connector-cli -- diff` is the measurement, and when this landed it
/// reported `945 artifacts up to date (54 providers checked)` — no drift, nothing rewritten.
fn canonical(operation: &Operation, printed: String) -> Result<String> {
    let parsed = flux_lang::parser::parse_cst(&printed);
    if !parsed.errors.is_empty() {
        return Err(Error::NotCanonical {
            operation: operation.id.clone(),
            reason: format!("it does not parse: {:?}", parsed.errors),
        });
    }
    flux_lang::format_cst::format_module(&parsed).ok_or_else(|| Error::NotCanonical {
        operation: operation.id.clone(),
        reason: "flux's own formatter could not re-print it, so no spelling of this operation is \
                 canonical"
            .to_string(),
    })
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

/// **A value an operator pinned at install time**, and the symbol carrying it — C-187.
///
/// Emitted exactly the way a templated `base_url` already is: the symbol is bound to a **string
/// literal holding the `{placeholder}`**, and a host substitutes the tenant's value into that
/// literal before the module runs. That is not a stylistic echo of `$base` but the mechanism
/// itself — `connector-pack` reads a connector's configuration variables off the braces surviving
/// in its emitted literals, and substitutes into literals *only*, precisely so that nothing a
/// caller passes can be substituted into. A pin written any other way would be invisible to that
/// resolution, and a pin written as a Flux literal *value* would put a tenant's zone id in this
/// repository, which is the category error the story rules out.
///
/// It is never a [`FluxParam`]: a pinned value that also reached the declared parameter list would
/// be a value a model could pass, and the operator's choice of tenant would become a suggestion.
struct Pinned<'a> {
    /// Where on the request it lands.
    position: Position,
    /// The name the vendor sees: the path template variable, the query parameter, the header.
    name: &'a str,
    /// **The placeholder the literal carries** — the field's one host-side slot.
    ///
    /// The same string as [`name`](Self::name) for every field binding one destination, which is
    /// why `Binding::Request` carries only one. A field that also binds its service's `base_url`
    /// variable (C-229) breaks the coincidence — Algolia's application id is `{app_id}` in the host
    /// and `X-Algolia-Application-Id` on the wire — and the *slot's* spelling is the one a host
    /// resolves, so it is the one the literal must carry in every position the value reaches.
    variable: String,
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
fn bind_parameters<'a>(operation: &'a Operation) -> Result<(Bindings<'a>, Symbols)> {
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

    Ok((
        Bindings {
            path,
            query,
            header,
            body,
            free_form,
            const_headers,
        },
        symbols,
    ))
}

/// The values an operator pinned for this operation's service, paired with their Flux symbols.
///
/// **Allocated from the allocator [`bind_parameters`] left behind**, after every parameter and every
/// constant header, for the reason that list already documents: adding a pin to a provider must not
/// rename a symbol that already travelled.
///
/// A [`Position::Path`] pin applies only to an operation whose path actually carries its
/// placeholder — Cloudflare's `cloudflare-zone-list` is the case, and it is the operation that
/// *discovers* zone ids, so requiring every operation to reference the pin would make it
/// unexpressible. A query or header pin applies to **every** operation of the service, and that is
/// the point rather than a simplification: a tenant scope honoured on some of a service's calls and
/// not others leaves the rest addressing a different tenant, which is the exact failure Vercel's
/// optional `teamId` demonstrates.
fn bind_pins<'a>(
    connector: &'a Connector,
    operation: &'a Operation,
    symbols: &mut Symbols,
) -> Result<Vec<Pinned<'a>>> {
    connector
        .config_of(&operation.service)
        .flat_map(|field| field.pins())
        .filter(|pin| match pin.position {
            Position::Path => {
                connector_spec::config::template_variables(&operation.path).contains(&pin.name)
            }
            Position::Query | Position::Header => true,
        })
        .map(|pin| {
            Ok(Pinned {
                position: pin.position,
                name: pin.name,
                variable: pin.variable.into_owned(),
                // Allocated from the **wire** name rather than the slot, so a field that grows a
                // second destination does not rename the symbol its first one already emitted, and
                // two destinations of one field get two symbols rather than colliding on one.
                symbol: symbols.allocate(&operation.id, pin.name)?,
            })
        })
        .collect()
}

/// The pins landing on one request position, in declaration order.
fn pins_at<'a, 'b>(pinned: &'b [Pinned<'a>], position: Position) -> Vec<&'b Pinned<'a>> {
    pinned
        .iter()
        .filter(|pin| pin.position == position)
        .collect()
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
    // Pins are deliberately absent and the signature is why they can be: an operator-pinned value is
    // not a parameter, so no call site has an argument to spell for it. See `Pinned`.
    let (bound, _) = bind_parameters(operation)?;
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
    check_credential_diversion(operation)?;

    // Kept whole rather than destructured: `request_body` needs every group, and passing them one
    // slice at a time is how a lowering grows an argument list nobody can read.
    let (bound, mut symbols) = bind_parameters(operation)?;
    let pinned = bind_pins(connector, operation, &mut symbols)?;
    let Bindings {
        path,
        query,
        header,
        body,
        free_form,
        const_headers,
    } = &bound;

    check_pins(operation, &bound, &pinned)?;
    check_query_values(operation, query)?;

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
        // A dotted or bracketed name with no `wire` cannot be read either way — see
        // `Error::NestedBodyField`. The bracket joined the dot when an index became a path step
        // (C-185): without this, `name = "items[0]"` and no `wire` would build an array out of a
        // name that may well be one the vendor spells with brackets.
        if bound.param.wire.is_none() && bound.param.name.contains(['.', '[', ']']) {
            return Err(Error::NestedBodyField {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
            });
        }
    }
    check_body_encoding(operation, body, free_form.as_ref())?;
    check_header_names(
        operation,
        header,
        const_headers,
        &pinned,
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
            body: request_body(connector, operation, &bound, &pinned)?,
            ..DraftAst::default()
        },
    })
}

/// Refuse query shapes for which neither the IR nor Flux states a wire convention — C-30.
fn check_query_values(operation: &Operation, query: &[Bound<'_>]) -> Result<()> {
    for bound in query {
        let ty = flux_type(&bound.param.schema);
        if !matches!(ty, TypeRef::String | TypeRef::Number | TypeRef::Bool) {
            return Err(Error::UnencodableQueryValue {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
                kind: ty.label(),
            });
        }
    }
    Ok(())
}

/// **A pin and a parameter may not claim one request slot** — C-187.
///
/// The repository's standing shape for a declaration it cannot honour: a refusal that names what
/// went wrong, rather than a silent precedence rule. `Error::ConstantHeaderParam` is the same
/// judgement one story earlier — `const` on a header parameter used to be dropped quietly, which
/// shipped connectors whose mandatory header was whatever the caller passed.
///
/// Honouring one side instead is not available. If the parameter wins, the pin is decoration and a
/// model chooses the tenant; if the pin wins, the operation declares an argument that goes nowhere
/// and a caller's value vanishes without a word. Both are requests a vendor answers `200` to, which
/// is the failure mode this emitter refuses everywhere else.
///
/// Header pins are absent here because [`check_header_names`] already claims every header name from
/// all three sources at once; a pin colliding there is an [`Error::HeaderConflict`] naming both.
fn check_pins(operation: &Operation, bound: &Bindings<'_>, pinned: &[Pinned<'_>]) -> Result<()> {
    for pin in pinned {
        let claimed = match pin.position {
            Position::Path => bound.path.iter().any(|b| wire_name(b.param) == pin.name),
            Position::Query => bound.query.iter().any(|b| wire_name(b.param) == pin.name),
            // Claimed by `check_header_names`, against every source at once.
            Position::Header => false,
        };
        if claimed {
            return Err(Error::PinnedValueConflict {
                operation: operation.id.clone(),
                position: pin.position.word(),
                name: pin.name.to_string(),
            });
        }
    }
    Ok(())
}

/// Everything a declared [`BodyEncoding`] must be true of before the body is assembled — C-144.
///
/// Only the non-default encoding is checked, and that is the whole compatibility story: an operation
/// that declares nothing reaches none of these branches, so every module this repository already
/// ships is emitted by exactly the code that emitted it before.
///
/// Each refusal exists because its alternative is a request the vendor answers `200` to. See
/// [`Error::UnencodableFormField`], [`Error::UnencodableFormBody`] and
/// [`Error::BodyEncodingWithoutBody`].
fn check_body_encoding(
    operation: &Operation,
    body: &[Bound<'_>],
    free_form: Option<&FreeFormBody<'_>>,
) -> Result<()> {
    let encoding = operation.params.body_encoding;
    if encoding.is_json() {
        return Ok(());
    }

    if free_form.is_some() {
        return Err(Error::UnencodableFormBody {
            operation: operation.id.clone(),
            encoding: encoding.tag(),
        });
    }
    if body.is_empty() {
        return Err(Error::BodyEncodingWithoutBody {
            operation: operation.id.clone(),
            encoding: encoding.tag(),
        });
    }

    for bound in body {
        let wire = wire_name(bound.param);
        // A `fmt` template is what assembles the pairs, so a brace in a key would be read as an
        // interpolation delimiter — `Symbols::allocate` makes the same check, but on the
        // caller-facing name, and it is the *wire* name that reaches this template.
        let reason = if wire.contains('.') {
            Some("its `wire` path is nested")
        } else if wire.contains('[') || wire.contains(']') {
            // C-185 gave a `wire` path array indices, and C-144's refusal of nesting has to cover
            // them: a form body is `fmt`-assembled text, so `items[0]=…` would reach the vendor as
            // a literal key. Some form parsers do read that as an array and none of the vendors
            // described here has been checked for it, which is precisely the guess this refuses.
            Some("its `wire` path indexes an array, which a form body cannot express")
        } else if wire.is_empty() {
            Some("its wire name is empty, which is not a form key")
        } else if wire.contains('{') || wire.contains('}') {
            Some("its wire name contains a brace, which would corrupt the form template")
        } else if matches!(
            bound.param.schema.get("type").and_then(|ty| ty.as_str()),
            Some("object" | "array")
        ) {
            // The value nests even though the key does not. Interpolated, it would reach the vendor
            // as JSON text inside a form pair — a shape no form parser reassembles.
            Some("its declared value is an object or an array")
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(Error::UnencodableFormField {
                operation: operation.id.clone(),
                name: bound.param.name.clone(),
                wire: wire.to_string(),
                reason,
            });
        }
    }
    Ok(())
}

/// Every header the request will carry, checked once: spellable, and claimed by one declaration.
///
/// The record `http.request` receives has one slot per name, so a second claim on a name overwrites
/// the first — silently, in an order nothing in the provider file makes visible. That is why this is
/// a refusal rather than a merge, and it is the header-side twin of [`Error::BodyPathConflict`].
///
/// Four sources can claim a name: the media type the emitter derives from the request body, a
/// caller-supplied `params.header`, a `const_headers` entry, and an operator-pinned configuration
/// value (C-187). The comparison is case-insensitive because HTTP field names are (RFC 9110 §5.1) —
/// a map keyed by spelling would hold `Notion-Version` and `notion-version` happily, and send the
/// header twice.
fn check_header_names(
    operation: &Operation,
    header: &[Bound<'_>],
    const_headers: &[ConstantHeader<'_>],
    pinned: &[Pinned<'_>],
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
    // Last, so a collision is reported against the declaration that was already there. A pinned
    // header carries an operator's value, so the same spelling rule applies: `http.request` builds a
    // `HeaderName` from it and errors on anything that is not an HTTP token.
    for pin in pins_at(pinned, Position::Header) {
        if !is_http_token(pin.name) {
            return Err(Error::BadHeaderName {
                operation: operation.id.clone(),
                name: pin.name.to_string(),
            });
        }
        claim(pin.name, "an operator-pinned configuration value")?;
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
///
/// # What C-186 changed, and what it deliberately did not
///
/// Neither the `risk` refusal nor the `idempotent`-on-`POST`/`PATCH` refusal is relaxed. **Both are
/// unconditional, exactly as they were.** C-186's first landing did relax the second one — it let a
/// `POST` claim `Idempotent` behind a justification — and that was wrong on flux's own terms:
/// `flux_spec::coherence`'s I3 records that `Idempotent` is what "licenses the dispatcher's op cache
/// to serve a stored result *instead of executing*". "Safe to repeat" and "safe to skip" are
/// different claims, and only the second is what `Idempotent` means. Purging a cache is the first.
///
/// What landed instead is a **tightening**, on the value flux actually reserves for this:
/// [`Idempotency::Conditional`] is flux's stated escape hatch for a mutation that is genuinely
/// replay-safe, and it was already permitted on every method here with nothing asked of it. Six
/// operations used it before C-186 with the condition recorded nowhere. A mutating `conditional`
/// must now state its condition, and the condition is refused where it means nothing.
/// **A credential-producing operation is refused rather than emitted** (C-136).
///
/// The body this module builds ends in `response = http.request(…)` / `return response`. For an
/// operation whose whole purpose is to mint a credential, that is a module that performs the login
/// and binds the raw token to a symbol a model can read — precisely what `AGENTS.md`
/// § Authentication contract forbids, and it would ship *beside* a `web/public/catalog.json` entry
/// promising the caller a handle. The executable artifact would be the wrong one of the two.
///
/// The diversion exists on the **host** path only: `connector_pack::mint` writes the secret through
/// a bound `CredentialStore` and returns the address. Flux has no such port, so there is no body
/// this function could produce instead — which is why this is a refusal and not a different
/// lowering.
///
/// Checked on the IR rather than only at the loader, for the reason [`check_write_metadata`] is:
/// **an IR assembled in memory never passes through `provider::load`**, and this is the guard that
/// must not be walkable-past. It is deliberately the *only* place the arrangement is refused —
/// putting it in the loader as well would make this one unreachable, and the emitter is where the
/// hazard actually is.
fn check_credential_diversion(operation: &Operation) -> Result<()> {
    let Some(produced) = &operation.produces_credential else {
        return Ok(());
    };
    Err(Error::CredentialProducingOperation {
        operation: operation.id.clone(),
        credential: produced.credential.clone(),
        secret: produced.secret.clone(),
    })
}

fn check_write_metadata(operation: &Operation) -> Result<()> {
    // Checked ahead of the `mutates` gate, because the methods this refuses are exactly the ones the
    // gate returns early on. A condition on a `GET` is where the field is most tempting and least
    // meaningful: nothing about a read needs conditioning.
    if operation.repeatable_because.is_some() && !mutates(operation.method) {
        return Err(Error::RepeatabilityConditionUnneeded {
            operation: operation.id.clone(),
            method: method_word(operation.method),
        });
    }
    // Likewise ahead of the gate for the same reason, and checked on the IR rather than only at the
    // loader because an IR assembled in memory never passes through `provider::load`.
    if operation.repeatable_because.is_some() && operation.idempotency != Idempotency::Conditional {
        return Err(Error::RepeatabilityConditionWithoutTheClaim {
            operation: operation.id.clone(),
            idempotency: idempotency_tag(operation.idempotency),
        });
    }
    if !mutates(operation.method) {
        return Ok(());
    }
    if operation.risk == Risk::Low {
        return Err(Error::WriteDeclaredLowRisk {
            operation: operation.id.clone(),
            method: method_word(operation.method),
        });
    }
    // `PUT` and `DELETE` *are* idempotent methods under RFC 9110 §9.2.2, so this repository has
    // always let them declare `idempotent`; `POST` and `PATCH` are not, and claiming otherwise makes
    // a `retry` around the call unsound.
    //
    // Worth knowing while reading this: the `PUT`/`DELETE` permission is in open conflict with
    // flux's I3, which does not consider the HTTP method at all — it refuses `Idempotent` on
    // anything consequence-bearing. Nine shipped `PUT`s sit in that gap today. Resolving it is a
    // decision about whose vocabulary wins across eight providers, not something to settle inside a
    // guard, so it is measured and filed rather than changed here — see
    // `crates/connector-pack/tests/metadata_coherence.rs`.
    if operation.idempotency == Idempotency::Idempotent
        && matches!(operation.method, HttpMethod::Post | HttpMethod::Patch)
    {
        return Err(Error::WriteDeclaredIdempotent {
            operation: operation.id.clone(),
            method: method_word(operation.method),
        });
    }
    // The tightening. `conditional` was the one metadata value a write could carry with nothing
    // asked of it, and flux's word for what makes it honest is "stated".
    if operation.idempotency == Idempotency::Conditional
        && operation.repeatability_condition().is_none()
    {
        return Err(Error::ConditionalWithoutItsCondition {
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
        // `expose true` is what surfaces the op to the model as an LLM tool — and it was a literal
        // here until C-413, which fused "this operation can be called" with "this operation is a
        // tool". Reading the declaration instead is what lets a connector cover a whole API without
        // spending 397 tool slots on it: an unexposed operation is still catalogued, still in the
        // manifest, and `connector-pack` still builds a request for it. Only this line is withheld.
        expose: operation.expose,
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
    pinned: &[Pinned<'_>],
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

    let pinned_query = pins_at(pinned, Position::Query);

    let mut template = String::from("{base}");
    template.push_str(&path_template(operation, path, pinned)?);

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
    ];

    // Bound between the base URL and the URL that reads them, and bound to a **literal holding the
    // placeholder** rather than to a value: a host substitutes into literals only, which is what
    // keeps a caller's parameter from ever being substituted into. See `Pinned`.
    //
    // The placeholder is the field's **slot**, not the pin's wire name. For every single-destination
    // field the two are the same string; for one that also composes its host (C-229) they differ,
    // and it is the slot a host resolves — so a pin carrying its own spelling would ask for a value
    // nobody was asked to supply, beside a `base_url` asking for the one they were.
    for pin in pinned {
        body.push(bind_string(&pin.symbol, &format!("{{{}}}", pin.variable)));
    }
    body.push(bind_fmt(URL, template));

    let mut request = BTreeMap::from([
        ("url".to_string(), Box::new(symbol(URL))),
        (
            "method".to_string(),
            Box::new(Node::Lit {
                value: serde_json::Value::String(method_word(operation.method).to_string()),
            }),
        ),
    ]);

    let query_fields: BTreeMap<String, Box<Node>> = query
        .iter()
        .map(|bound| {
            (
                wire_name(bound.param).to_owned(),
                Box::new(symbol(&bound.symbol)),
            )
        })
        .chain(
            pinned_query
                .iter()
                .map(|pin| (pin.name.to_owned(), Box::new(symbol(&pin.symbol)))),
        )
        .collect();
    if !query_fields.is_empty() {
        request.insert(
            "query".to_owned(),
            Box::new(Node::Obj {
                fields: query_fields,
            }),
        );
    }

    // Headers: the media type the payload is encoded in, the ones the vendor fixes, plus whatever
    // the caller supplies. Auth headers are deliberately absent — C-10 adds the credential
    // reference, and a constant header may not carry one (`connector_spec`'s loader refuses it).
    let has_body = !body_params.is_empty() || free_form.is_some();
    let mut headers: BTreeMap<String, Box<Node>> = BTreeMap::new();
    if has_body {
        // Derived from the operation's declaration, never authored as a header: a provider that could
        // write `content-type` itself could claim an encoding this emitter does not produce, which is
        // worse than the honest single-encoding limitation C-144 replaced. `check_header_names`
        // refuses a constant header claiming the name for exactly that reason.
        body.push(bind_string(
            CONTENT_TYPE,
            operation.params.body_encoding.media_type(),
        ));
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
    // An operator-pinned header. Its symbol is already bound, above the URL, because the same
    // literal is what a host substitutes into whichever position the pin lands on.
    for pin in pins_at(pinned, Position::Header) {
        headers.insert(pin.name.to_string(), Box::new(symbol(&pin.symbol)));
    }
    if !headers.is_empty() {
        request.insert(
            "headers".to_string(),
            Box::new(Node::Obj { fields: headers }),
        );
    }

    // The body, assembled and passed **by symbol**. That is load-bearing rather than stylistic:
    // `http.request` reads its `body` argument with `Value::as_str`
    // (`../flux/crates/flux-web/src/http.rs:183-186`), so an inline record arrives as a JSON object
    // and is silently dropped, whereas a bound record is stored as canonical JSON *text* and
    // arrives intact. A `form` body is text from the start — see `form_payload`.
    if !body_params.is_empty() {
        for bound in body_params {
            if let Some(value) = constant(bound.param) {
                body.push(bind_lit(&bound.symbol, value.clone()));
            }
        }
        if operation.params.body_encoding.is_json() {
            body.push(Node::Bind {
                name: SymbolName(PAYLOAD.to_string()),
                value: Box::new(body_tree(operation, body_params)?.into_node()),
                ty: None,
                effect: None,
            });
        } else {
            body.extend(form_payload(body_params));
        }
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

/// One step of a parsed [`Param::wire`] path: an object key, or one element of the array that key
/// holds — C-185.
///
/// `personalizations[0].to[0].email` is five steps, and the two kinds are what a `BodyNode` matches
/// on to decide whether the position it is standing at is a record or a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step<'a> {
    /// A JSON object key.
    Key(&'a str),
    /// A position in the array held by the key immediately before it.
    Index(usize),
}

/// Parse one `wire` path into steps, refusing every spelling that is not one of the two.
///
/// The grammar is deliberately small: `key` or `key[0]`, joined by dots. A segment that is neither
/// is refused rather than absorbed into an object key, because an absorbed bracket produces
/// `{"items[": …}` — accepted by the vendor, ignored, answered 200.
fn wire_steps<'a>(operation: &Operation, param: &Param, wire: &'a str) -> Result<Vec<Step<'a>>> {
    let bad_path = || Error::BadWirePath {
        operation: operation.id.clone(),
        name: param.name.clone(),
        wire: wire.to_string(),
    };
    let bad_index = |segment: &str, reason: &'static str| Error::BadArrayIndex {
        operation: operation.id.clone(),
        name: param.name.clone(),
        wire: wire.to_string(),
        segment: segment.to_string(),
        reason,
    };

    let mut steps = Vec::new();
    for segment in wire.split('.') {
        let (key, brackets) = match segment.find('[') {
            Some(at) => segment.split_at(at),
            None => (segment, ""),
        };
        if key.is_empty() {
            // Covers `""`, `".a"`, `"a."`, `"a..b"` exactly as before — and `"[0].x"`, which is a
            // body whose *root* is an array. See the module documentation for why that stays out.
            return Err(bad_path());
        }
        if key.contains(']') {
            return Err(bad_index(key, "it closes a bracket it never opened"));
        }
        if key.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::NumericWirePathSegment {
                operation: operation.id.clone(),
                name: param.name.clone(),
                wire: wire.to_string(),
                segment: key.to_string(),
                segment_bracketed: format!("[{key}]"),
            });
        }
        steps.push(Step::Key(key));

        if brackets.is_empty() {
            continue;
        }
        let inner = &brackets[1..];
        let Some(close) = inner.find(']') else {
            return Err(bad_index(brackets, "its `[` is never closed"));
        };
        let (digits, after) = inner.split_at(close);
        let after = &after[1..];
        if !after.is_empty() {
            return Err(bad_index(
                brackets,
                if after.starts_with('[') {
                    "an array whose elements are themselves arrays has no spelling here"
                } else {
                    "text follows the closing `]`"
                },
            ));
        }
        if digits.is_empty() {
            return Err(bad_index(brackets, "its brackets hold no index"));
        }
        if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(bad_index(
                brackets,
                "an index is a decimal position from `[0]` up",
            ));
        }
        if digits.len() > 1 && digits.starts_with('0') {
            return Err(bad_index(
                brackets,
                "a leading zero would give one position two spellings",
            ));
        }
        let index = digits
            .parse::<usize>()
            .map_err(|_| bad_index(brackets, "the index is larger than this machine counts to"))?;
        steps.push(Step::Index(index));
    }
    Ok(steps)
}

/// The steps back in `wire` spelling, so a refusal names the path an author wrote.
fn render_steps(steps: &[Step<'_>]) -> String {
    let mut rendered = String::new();
    for step in steps {
        match step {
            Step::Key(key) => {
                if !rendered.is_empty() {
                    rendered.push('.');
                }
                rendered.push_str(key);
            }
            Step::Index(index) => rendered.push_str(&format!("[{index}]")),
        }
    }
    rendered
}

/// The request body under construction: a JSON tree, keyed by wire path step.
///
/// A tree rather than a flat map because a vendor's body nests — Zendesk's comment text lives at
/// `ticket.comment.body` — and the flat spelling `{"ticket.comment.body": …}` is a request Zendesk
/// accepts and ignores. `BTreeMap` at every level is what makes the emitted record deterministic
/// without sorting anything at emit time; for [`BodyNode::Elements`] it additionally puts the
/// declared indices in position order, which is the order the array is emitted in.
#[derive(Debug)]
enum BodyNode {
    /// A caller-supplied (or constant) value, carried by this Flux symbol.
    Leaf(String),
    /// An object of further paths.
    Branch(BTreeMap<String, BodyNode>),
    /// An array, keyed by the position each declared element occupies — C-185. Held sparsely while
    /// the tree is built and checked dense before it is lowered, so `items[0]` and `items[2]` can
    /// be refused by naming the hole rather than by silently closing it.
    Elements(BTreeMap<usize, BodyNode>),
}

impl BodyNode {
    /// An empty container of the kind `step` needs to stand on.
    fn empty_for(step: &Step<'_>) -> BodyNode {
        match step {
            Step::Key(_) => BodyNode::Branch(BTreeMap::new()),
            Step::Index(_) => BodyNode::Elements(BTreeMap::new()),
        }
    }

    /// Place `symbol_name` at `steps`.
    ///
    /// `Err(depth)` reports that the first `depth` steps of the path are contested — they already
    /// hold a value this field would have to live *inside*, they hold the other kind of container,
    /// or another field already claimed exactly them. The caller turns that into
    /// [`Error::BodyPathConflict`]; reporting the depth rather than a message is what lets one
    /// recursive step stay ignorant of the whole path.
    fn insert(&mut self, steps: &[Step<'_>], symbol_name: &str) -> std::result::Result<(), usize> {
        let (head, rest) = steps.split_first().expect("a wire path has a step");
        let slot = match (&mut *self, head) {
            (BodyNode::Branch(children), Step::Key(key)) => {
                if rest.is_empty() {
                    if children.contains_key(*key) {
                        return Err(1);
                    }
                    children.insert((*key).to_string(), BodyNode::Leaf(symbol_name.to_string()));
                    return Ok(());
                }
                children
                    .entry((*key).to_string())
                    .or_insert_with(|| BodyNode::empty_for(&rest[0]))
            }
            (BodyNode::Elements(items), Step::Index(index)) => {
                if rest.is_empty() {
                    if items.contains_key(index) {
                        return Err(1);
                    }
                    items.insert(*index, BodyNode::Leaf(symbol_name.to_string()));
                    return Ok(());
                }
                items
                    .entry(*index)
                    .or_insert_with(|| BodyNode::empty_for(&rest[0]))
            }
            // Whatever occupies this position is not what this path needs it to be: a value the
            // path wants to descend into, an object the path indexes, or an array the path keys.
            // The caller adds the depth it was reached at.
            _ => return Err(0),
        };
        slot.insert(rest, symbol_name).map_err(|depth| depth + 1)
    }

    /// Refuse any array whose declared indices are not `0..n` — C-185.
    ///
    /// Run once over the finished tree rather than at insert time, because "the indices are dense"
    /// is a property of the whole field set and no single field can be the one that breaks it.
    fn check_dense(&self, operation: &Operation, path: &str) -> Result<()> {
        match self {
            BodyNode::Leaf(_) => Ok(()),
            BodyNode::Branch(children) => children.iter().try_for_each(|(key, child)| {
                let below = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                child.check_dense(operation, &below)
            }),
            BodyNode::Elements(items) => {
                for (position, index) in items.keys().enumerate() {
                    if *index != position {
                        return Err(Error::SparseBodyArray {
                            operation: operation.id.clone(),
                            path: path.to_string(),
                            missing: position,
                            declared: items
                                .keys()
                                .map(|index| format!("[{index}]"))
                                .collect::<Vec<_>>()
                                .join(", "),
                        });
                    }
                }
                items.iter().try_for_each(|(index, child)| {
                    child.check_dense(operation, &format!("{path}[{index}]"))
                })
            }
        }
    }

    /// Lower the tree into the nested Flux value the payload is bound to.
    fn into_node(self) -> Node {
        match self {
            BodyNode::Leaf(symbol_name) => symbol(&symbol_name),
            BodyNode::Branch(children) => Node::Obj {
                fields: children
                    .into_iter()
                    .map(|(key, child)| (key, Box::new(child.into_node())))
                    .collect(),
            },
            // `Node::List` is flux's own list constructor, so an array is a projection of Flux
            // rather than a shape this crate invented — the same standing every other node here
            // has. Its items are in `BTreeMap<usize, _>` order, which is index order.
            BodyNode::Elements(items) => Node::List {
                items: items
                    .into_values()
                    .map(BodyNode::into_node)
                    .collect::<Vec<_>>(),
            },
        }
    }
}

/// Assemble every body field into one tree, each at the JSON path its [`Param::wire`] names.
///
/// Every way a path set can be incoherent is refused rather than resolved, because each resolution
/// silently drops or invents something the author did not declare: an empty segment
/// ([`Error::BadWirePath`]), a malformed index ([`Error::BadArrayIndex`]), a bare numeric segment
/// ([`Error::NumericWirePathSegment`]), a path that needs to be two shapes at once
/// ([`Error::BodyPathConflict`]), and an array with a hole in it ([`Error::SparseBodyArray`]).
fn body_tree(operation: &Operation, body_params: &[Bound<'_>]) -> Result<BodyNode> {
    let mut root = BodyNode::Branch(BTreeMap::new());
    // Every path placed so far, so a conflict can name *both* fields rather than only the one that
    // arrived second.
    let mut placed: Vec<(&str, &str)> = Vec::new();

    for bound in body_params {
        let wire = wire_name(bound.param);
        let steps = wire_steps(operation, bound.param, wire)?;

        match root.insert(&steps, &bound.symbol) {
            Ok(()) => placed.push((wire, bound.param.name.as_str())),
            Err(depth) => {
                let path = render_steps(&steps[..depth]);
                // The field already occupying that path is the one whose own path is it, or lies
                // under it — one step further down is a `.` for an object and a `[` for an array.
                let first = placed
                    .iter()
                    .find(|(placed_wire, _)| {
                        *placed_wire == path
                            || placed_wire.starts_with(&format!("{path}."))
                            || placed_wire.starts_with(&format!("{path}["))
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

    root.check_dense(operation, "")?;
    Ok(root)
}

/// A `form` body: `key=value` pairs assembled as **text**, not as a record — C-144.
///
/// # Why `fmt` and not an encoder
///
/// `http.request` reads `body` with `Value::as_str` and forwards the bytes verbatim, so a form body
/// has to arrive as text. Under JSON that text comes from flux — a bound record is stored as
/// canonical JSON, and a caller-supplied one is canonicalized by `parse($body, as: "json")`. **flux
/// has no equivalent for a form body**: `parse`'s `as_type` is restricted to
/// `f64`/`i64`/`bool`/`json`/`string` by flux-lang's own analyzer, no node or `expr` function
/// serializes to a form, and nothing in flux's core catalogue percent-encodes. So `fmt` — the same
/// construction used before structured query fields were available — is the only one available,
/// and hand-rolling percent-encoding out of `replace` chains in emitted Flux is not an improvement
/// on it but a connector-specific DSL this repository refuses.
///
/// **The cost, stated plainly: form values are interpolated verbatim.** A value carrying `&` or `=`
/// corrupts the body and can inject a field. Query values no longer share that gap (C-30); the form
/// fix needs its own structured runtime field or a Flux-side encoder.
///
/// # Which pairs are guarded
///
/// A **required** field, and a field pinned by a `const`, always travels: it goes into the opening
/// template. An **optional** one is guarded by `when`, for the reason the query string is: flux has
/// no absent argument, so an unsupplied optional arrives as null, and interpolating it would send the
/// vendor the literal text `note=null` — a value, not an omission. Only a body whose *every* field is
/// optional needs [`FORM_SEP`]; with one always-present pair the separator is a literal `&`.
fn form_payload(body_params: &[Bound<'_>]) -> Vec<Node> {
    let pair = |bound: &Bound<'_>| format!("{}={{{}}}", wire_name(bound.param), bound.symbol);
    let (always, guarded): (Vec<&Bound<'_>>, Vec<&Bound<'_>>) = body_params
        .iter()
        .partition(|bound| bound.param.required || constant(bound.param).is_some());

    // The separator only has to be carried in a symbol when nothing else guarantees the payload is
    // non-empty *and* more than one guarded pair could open it.
    let carried_sep = always.is_empty() && guarded.len() > 1;

    let mut out = Vec::new();
    if always.is_empty() {
        out.push(bind_string(PAYLOAD, ""));
        if carried_sep {
            out.push(bind_string(FORM_SEP, ""));
        }
    } else {
        out.push(bind_fmt(
            PAYLOAD,
            always
                .iter()
                .map(|bound| pair(bound))
                .collect::<Vec<_>>()
                .join("&"),
        ));
    }

    for (i, bound) in guarded.iter().enumerate() {
        let separator = match (always.is_empty(), carried_sep) {
            (false, _) => "&".to_string(),
            (true, true) => format!("{{{FORM_SEP}}}"),
            (true, false) => String::new(),
        };
        let mut then = vec![bind_fmt(
            PAYLOAD,
            format!("{{{PAYLOAD}}}{separator}{}", pair(bound)),
        )];
        // The last pair never needs to hand a separator on — the same economy the query string makes.
        if carried_sep && i + 1 < guarded.len() {
            then.push(bind_string(FORM_SEP, "&"));
        }
        out.push(Node::When {
            cond: Box::new(symbol(&bound.symbol)),
            then,
            otherwise: Vec::new(),
        });
    }

    out
}

/// The vendor path template with each `{wire_name}` rewritten to `{symbol_name}`, so the `fmt`
/// interpolation resolves against the symbols the op actually declares.
///
/// Both directions of mismatch are refused: a placeholder with no declared parameter would
/// interpolate to a literal `{name}` in the URL, and a declared path parameter that never appears
/// in the template could never travel.
fn path_template(
    operation: &Operation,
    path: &[Bound<'_>],
    pinned: &[Pinned<'_>],
) -> Result<String> {
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
        // A caller's argument first, then an operator's pin. The two can never both answer: a
        // segment claimed by both is refused by `check_pins` before this runs, so the order here
        // decides nothing and is only the order the two were introduced in.
        let symbol_name = match path.iter().position(|b| wire_name(b.param) == wire) {
            Some(index) => {
                used[index] = true;
                path[index].symbol.clone()
            }
            None => pins_at(pinned, Position::Path)
                .into_iter()
                .find(|pin| pin.name == wire)
                .map(|pin| pin.symbol.clone())
                .ok_or_else(|| Error::UndeclaredPathParam {
                    operation: operation.id.clone(),
                    path: operation.path.clone(),
                    name: wire.to_string(),
                })?,
        };
        out.push_str(&format!("{{{symbol_name}}}"));
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
            semantic_effects: Vec::new(),
            repeatable_because: None,
            expose: true,
            auth: None,
            params: ParamSet {
                path: path_params,
                ..ParamSet::default()
            },
            response_schema: None,
            credential_response: Vec::new(),
            produces_credential: None,
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
            runtime: connector_spec::Runtime::Http,
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
        assert_eq!(
            path_template(&op, &bound, &[]).unwrap(),
            "/v2/calls/{call_id}"
        );
    }

    #[test]
    fn a_missing_leading_slash_is_supplied() {
        let op = operation("v2/agents", Vec::new());
        assert_eq!(path_template(&op, &[], &[]).unwrap(), "/v2/agents");
    }

    /// A placeholder nothing declares would interpolate to a literal `{id}` in the request URL.
    #[test]
    fn an_undeclared_path_placeholder_is_refused() {
        let op = operation("/v2/calls/{id}", Vec::new());
        assert!(matches!(
            path_template(&op, &[], &[]),
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
            path_template(&op, &bound, &[]),
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

    /// The array twin of the test above — C-185. Asserted on the AST for the same reason: what a
    /// bracketed index means is `Node::List`, and a formatting change must not be able to make that
    /// pass or fail.
    #[test]
    fn indexed_wire_paths_assemble_arrays_of_records() {
        let op = operation("/a", Vec::new());
        let params = vec![
            body_param("to_address", Some("personalizations[0].to[0].email")),
            body_param("body_text", Some("content[0].value")),
        ];
        let bound = bind_all(&op, &params);
        let record = body_tree(&op, &bound)
            .expect("an envelope is a coherent path set")
            .into_node();

        let Node::Obj { fields: root } = &record else {
            panic!("a body is a record");
        };
        assert_eq!(
            root.keys().collect::<Vec<_>>(),
            ["content", "personalizations"]
        );

        let Node::List { items } = root["personalizations"].as_ref() else {
            panic!("`personalizations` holds an array, not an object keyed by digits");
        };
        assert_eq!(items.len(), 1, "one declared index is a one-element array");
        let Node::Obj { fields: element } = &items[0] else {
            panic!("the element is a record");
        };
        let Node::List { items: recipients } = element["to"].as_ref() else {
            panic!("`to` holds a further array");
        };
        let Node::Obj { fields: recipient } = &recipients[0] else {
            panic!("a recipient is a record");
        };
        assert!(matches!(
            recipient["email"].as_ref(),
            Node::Var { name } if name.0 == "to_address"
        ));
    }

    /// The declared indices *are* the positions, in order — the tree is keyed by index rather than
    /// by arrival, so two fields declared in either order build the same array.
    #[test]
    fn an_arrays_positions_are_its_declared_indices_in_order() {
        let op = operation("/a", Vec::new());
        let first = ("first", "items[0].id");
        let second = ("second", "items[1].id");
        for order in [[first, second], [second, first]] {
            let params: Vec<Param> = order
                .iter()
                .map(|(name, wire)| body_param(name, Some(wire)))
                .collect();
            let bound = bind_all(&op, &params);
            let record = body_tree(&op, &bound).expect("two elements").into_node();

            let Node::Obj { fields: root } = &record else {
                panic!("a body is a record");
            };
            let Node::List { items } = root["items"].as_ref() else {
                panic!("`items` holds an array");
            };
            assert_eq!(items.len(), 2, "two declared indices are two elements");
            let names: Vec<&str> = items
                .iter()
                .map(|item| {
                    let Node::Obj { fields } = item else {
                        panic!("an element is a record");
                    };
                    let Node::Var { name } = fields["id"].as_ref() else {
                        panic!("an element's field is a symbol");
                    };
                    name.0.as_str()
                })
                .collect();
            // `second` is declared first in one of the two orderings; the array is the same either
            // way, because the index decides the position and the declaration order does not.
            assert_eq!(
                names,
                ["first", "second"],
                "element order follows the declared index, not the declaration order"
            );
        }
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
            legacy: false,
            description: String::new(),
            base_url: Some("https://gmail.googleapis.com/".to_string()),
            api_version: Some("v1".to_string()),
            roles: Vec::new(),
            tags: Vec::new(),
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
