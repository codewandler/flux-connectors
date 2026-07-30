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
//! op zendesk-ticket-comment-list(ticket_id: Number, page: Number, per_page: Number) -> Any
//!   description "List one Zendesk ticket's comments."
//!   risk "low"
//!   idempotency "idempotent"
//!   effects ["network"]
//!   expose true
//!
//!   $base = "https://example.zendesk.com"
//!   $url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
//!   $sep = "?"
//!   when $page
//!     $url = fmt("{url}{sep}page={page}")
//!     $sep = "&"
//!   when $per_page
//!     $url = fmt("{url}{sep}per_page={per_page}")
//!   do http.request { method: "GET", url: $url }
//! ```
//!
//! Four decisions in that are worth stating, because later stories build on them.
//!
//! **`$base` is a seam, not decoration.** The connector's base URL is bound once and interpolated,
//! so C-10 has a single statement to replace when the endpoint starts coming from operator config
//! instead of from the IR. It is a literal today.
//!
//! **`risk` and `idempotency` come from the IR.** flux's approval gate reads them, and the IR makes
//! both mandatory precisely so they cannot be decided by silence. Nothing here defaults them.
//!
//! **Required query parameters go in the URL template; optional ones are guarded.** An unbound
//! `{name}` placeholder is left *verbatim* in the string by flux's interpolator
//! (`interpolate_str`), so interpolating an unsupplied filter would send the vendor a literal
//! `?page={page}`. A `when $page` guard is what makes "not supplied" mean "not sent", and `$sep`
//! carries the `?`/`&` that only the first surviving parameter needs.
//!
//! **No credential is emitted.** Auth is C-10 and is deliberately absent rather than stubbed: an
//! invented placeholder marker would be a second thing to migrate.
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

/// Emit `operation` as a formatted Flux `op` declaration, ready to concatenate into a module.
///
/// The returned text is canonical: it parses, and flux-lang's own formatter leaves it unchanged.
/// `tests/op_emitter.rs` asserts both rather than trusting them.
///
/// # Scope
///
/// This is C-8's slice — an HTTP call whose parameters travel in the path and the query string.
/// Request bodies (C-9), caller-supplied headers, auth (C-10) and quirks (C-12) are refused or
/// omitted rather than half-emitted; see [`Error`].
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
    if !operation.params.body.is_empty() {
        return Err(Error::OutOfSlice {
            operation: operation.id.clone(),
            feature: "request body parameters",
        });
    }
    if !operation.params.header.is_empty() {
        return Err(Error::OutOfSlice {
            operation: operation.id.clone(),
            feature: "caller-supplied request headers",
        });
    }

    // Path first, then query — the IR's own request-position order, so the declared parameter list
    // is stable across regeneration.
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

    let params: Vec<FluxParam> = path
        .iter()
        .chain(&query)
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
            body: request_body(connector, operation, &path, &query)?,
            ..DraftAst::default()
        },
    })
}

/// The `description`/`risk`/`idempotency`/`effects`/`expose` block — the `ToolSpec` surface flux
/// exposes to a model, and the fields its approval gate reads.
fn metadata(operation: &Operation) -> Result<CompositeOpMeta> {
    Ok(CompositeOpMeta {
        description: operation.description.clone(),
        risk: from_tag(risk_tag(operation.risk))?,
        idempotency: from_tag(idempotency_tag(operation.idempotency))?,
        // Every generated op makes an HTTP request; nothing else it does is an effect flux tracks.
        effects: vec![from_tag("network")?],
        // `expose true` is what surfaces the op to the model as an LLM tool.
        expose: true,
        ..CompositeOpMeta::default()
    })
}

/// The op body: bind the base URL, assemble the request URL, issue the request.
fn request_body(
    connector: &Connector,
    operation: &Operation,
    path: &[Bound<'_>],
    query: &[Bound<'_>],
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

    // No credentials: the request goes out bare, and C-10 adds the auth reference.
    body.push(Node::Call {
        op: "http.request".to_string(),
        args: vec![Node::Obj {
            fields: BTreeMap::from([
                (
                    "url".to_string(),
                    Box::new(Node::Var {
                        name: SymbolName(URL.to_string()),
                    }),
                ),
                (
                    "method".to_string(),
                    Box::new(Node::Lit {
                        value: serde_json::Value::String(method_word(operation.method).to_string()),
                    }),
                ),
            ]),
        }],
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

/// `$name = "<literal>"`.
fn bind_string(name: &str, value: &str) -> Node {
    Node::Bind {
        name: SymbolName(name.to_string()),
        value: Box::new(Node::Lit {
            value: serde_json::Value::String(value.to_string()),
        }),
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
