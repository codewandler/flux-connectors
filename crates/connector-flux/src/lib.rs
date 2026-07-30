//! Emits Flux-Lang modules from the connector IR.
//!
//! Codegen builds real [`flux_lang`] AST nodes and formats them with flux-lang's own formatter, so
//! unparseable or non-canonical output is structurally impossible. **Never emit Flux through string
//! templates** — that convention is stated in AGENTS.md, and this crate is where it is enforced.
//!
//! # Where things live
//!
//! - [`emit_operation`] — one IR [`connector_spec::Operation`] becomes one formatted `op`
//!   declaration. Start there; the module docs on `op` describe the emitted shape and why.
//! - `names` — the wire-name/symbol-name mapping. A vendor may call a parameter `time.start`; Flux
//!   may not, and the two names are kept apart rather than reconciled by mangling.
//! - `types` — JSON Schema to Flux `TypeRef`, **including the documented `Any` fallback** for the
//!   shapes Flux cannot express.
//! - [`highlight`] — the other direction: formatted Flux rendered as a syntax-highlighted SVG,
//!   coloured by flux-lang's own CST classifier (C-45). It lives beside the emitter because both
//!   sides of "what Flux looks like" must come from flux-lang and never from a local imitation of
//!   its grammar.

pub mod highlight;
mod names;
mod op;
mod types;

pub use op::emit_operation;

/// flux-lang's own token classification, re-exported so a consumer can name it without depending on
/// flux-lang directly — `connector-cli` in particular does not, deliberately.
pub use flux_lang::highlight::HighlightClass;

/// Everything that can stop an IR operation from becoming a Flux `op`.
///
/// Every variant is a refusal, not a degradation: the alternative to each is emitting a request
/// that silently drops something the vendor needs.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The operation uses something this emitter does not cover yet — auth is C-10, and compiling
    /// quirks (pagination, rate limits) into control flow is C-12.
    #[error("operation `{operation}`: {feature} are outside this emitter's slice")]
    OutOfSlice {
        /// The operation id.
        operation: String,
        /// What could not be emitted.
        feature: &'static str,
    },

    /// A body field whose **caller-facing name** looks like a JSON path, with no `wire` to say
    /// whether it is one.
    ///
    /// The path a body field occupies is [`connector_spec::Param::wire`]'s job. A dotted `name` and
    /// no `wire` is ambiguous in the one direction that cannot be guessed at: it means either "a
    /// field literally called `presence.name` at the root of the body" or "the `name` field inside
    /// `presence`", and the two produce different requests. Emitting the dotted spelling as a
    /// literal JSON key yields `{"presence.name": …}`, which a vendor accepts and ignores — the
    /// worst failure available, because it answers 200.
    ///
    /// So it is refused, and the fix is one line in the provider file.
    #[error(
        "operation `{operation}`: body field `{name}` has a dotted name and no `wire`, so whether \
         it is one field called `\"{name}\"` or a nested path is undecidable. A body field's JSON \
         path is declared with `wire` — write `wire = \"{name}\"` and give the field a \
         caller-facing `name`. Emitted as-is it would be the literal key `\"{name}\"`, which the \
         vendor accepts and ignores"
    )]
    NestedBodyField {
        /// The operation id.
        operation: String,
        /// The dotted body field name.
        name: String,
    },

    /// A `wire` path with an empty segment — `"a..b"`, `".a"`, `"a."`, or the empty string.
    ///
    /// Every segment is a JSON object key, and an empty key is not one a vendor ever means. Left
    /// alone it would produce `{"a": {"": {"b": …}}}`, which is once again a request that is
    /// accepted and ignored.
    #[error(
        "operation `{operation}`: body field `{name}` declares `wire = \"{wire}\"`, which has an \
         empty path segment — every segment between dots is a JSON object key"
    )]
    BadWirePath {
        /// The operation id.
        operation: String,
        /// The caller-facing field name.
        name: String,
        /// The malformed path.
        wire: String,
    },

    /// Two body fields whose wire paths cannot both exist: one occupies a JSON path that the other
    /// needs as an object, or two fields claim the same path outright.
    ///
    /// `ticket.comment` and `ticket.comment.body` are not composable — the first says
    /// `comment` holds the caller's value, the second says it holds an object. Emitting either
    /// silently discards the other, so the operation is refused instead of one field being dropped.
    #[error(
        "operation `{operation}`: body fields `{first}` and `{second}` claim conflicting wire \
         paths — `{path}` cannot be both a value and an object. One of the two would be silently \
         dropped from the request"
    )]
    BodyPathConflict {
        /// The operation id.
        operation: String,
        /// The field already occupying the path.
        first: String,
        /// The field that collided with it.
        second: String,
        /// The contested path.
        path: String,
    },

    /// An operation declares both named body fields and a free-form `body_schema`.
    ///
    /// "The body is these fields" and "the body is this schema" are two answers to one question,
    /// and nothing in the IR states how to merge them — whether the schema is the envelope the
    /// fields sit in, or a sibling, or a replacement. Guessing would send one of the two and drop
    /// the other without saying so.
    #[error(
        "operation `{operation}`: declares both `params.body` fields and `params.body_schema`, and \
         nothing states how to merge them. A body is assembled from named fields *or* supplied \
         whole by the caller — declare one of the two"
    )]
    AmbiguousBody {
        /// The operation id.
        operation: String,
    },

    /// A request-changing method declared the risk of a read.
    ///
    /// flux's approval gate reads `risk`, and `low` is the tier that passes without a human. A
    /// `POST`/`PUT`/`PATCH`/`DELETE` changes state the vendor owns, which is not something this
    /// emitter can certify as unsurprising — [`connector_spec::Risk::Low`] is documented as "reads,
    /// and writes that cannot surprise anyone". Refused rather than quietly raised: a silent
    /// correction would hide the authoring mistake that produced it, and the IR omits `Default` on
    /// both fields precisely so neither is decided by silence.
    #[error(
        "operation `{operation}`: a {method} changes state the vendor owns and may not declare \
         `risk = \"low\"` — flux's approval gate waves `low` through without a human. Declare the \
         risk this write actually carries"
    )]
    WriteDeclaredLowRisk {
        /// The operation id.
        operation: String,
        /// The HTTP method, as `http.request` spells it.
        method: &'static str,
    },

    /// A `POST` or `PATCH` declared itself idempotent.
    ///
    /// Neither method is idempotent under RFC 9110 §9.2.2, and `idempotency` is what tells flux
    /// whether wrapping the call in a `retry` is sound — the field's own IR documentation puts it
    /// as "guessing is how a retry turns one charge into three". `PUT` and `DELETE` *are* idempotent
    /// by RFC and are left alone.
    #[error(
        "operation `{operation}`: a {method} is not an idempotent method (RFC 9110 §9.2.2) and may \
         not declare `idempotency = \"idempotent\"` — flux would treat a retry around it as safe. \
         Use `non_idempotent`, or `conditional` when the caller supplies a key or stamp"
    )]
    WriteDeclaredIdempotent {
        /// The operation id.
        operation: String,
        /// The HTTP method, as `http.request` spells it.
        method: &'static str,
    },

    /// The operation id cannot be spelled as a Flux composite-op **declaration** name.
    ///
    /// This one is worth reading twice, because the pipeline design assumes otherwise. A `call`
    /// site accepts a dotted op name (`flux_lang::ast::is_valid_op_name`), but a **declaration**
    /// does not: `flux_lang`'s `decl_name` grammar admits only ASCII alphanumerics, `_` and `-`,
    /// and flux's own composite loader agrees (`../flux/crates/flux-flow/src/composites.rs:340`,
    /// *"is not filename-safe"*). So `op zendesk.ticket.show` — the form used throughout
    /// [connector-pipeline.md](../../../docs/designs/connector-pipeline.md) and in
    /// [C-23](../../../docs/stories/C-23-operation-naming-contract.md) — does not parse today.
    ///
    /// Deciding what the public name becomes instead is C-23's job, not this emitter's, so an
    /// undeclarable id is refused here rather than quietly rewritten. Silently rewriting it is
    /// precisely the failure C-23 exists to prevent.
    #[error(
        "operation `{operation}`: a Flux `op` declaration name may only contain ASCII letters, \
         digits, `_` and `-` — a dotted id cannot be declared (see C-23, the operation naming \
         contract)"
    )]
    UnspellableOperationId {
        /// The operation id as the IR carries it.
        operation: String,
    },

    /// A vendor parameter name cannot be carried into generated Flux at all.
    #[error(
        "operation `{operation}`: parameter name `{name}` cannot be carried into Flux — {reason}"
    )]
    BadParamName {
        /// The operation id.
        operation: String,
        /// The vendor's parameter name.
        name: String,
        /// Why it cannot travel.
        reason: &'static str,
    },

    /// The path template names a parameter the operation does not declare, so nothing could ever
    /// substitute for it.
    #[error("operation `{operation}`: path `{path}` references `{{{name}}}`, which is not a declared path parameter")]
    UndeclaredPathParam {
        /// The operation id.
        operation: String,
        /// The path template.
        path: String,
        /// The unresolvable placeholder.
        name: String,
    },

    /// The operation declares a path parameter that never appears in its path template, so the
    /// caller's value would be silently discarded.
    #[error("operation `{operation}`: path parameter `{name}` never appears in path `{path}`")]
    UnusedPathParam {
        /// The operation id.
        operation: String,
        /// The path template.
        path: String,
        /// The parameter with nowhere to go.
        name: String,
    },

    /// A metadata tag this crate produced was not one flux-lang accepts — reachable only if the
    /// flux-lang pin changes the `risk`/`idempotency`/`effects` vocabulary out from under us, which
    /// is exactly when it should be loud.
    #[error("flux-lang rejected the metadata tag `{tag}`: {source}")]
    UnknownMetadataTag {
        /// The tag this crate emitted.
        tag: &'static str,
        /// The decoding failure.
        #[source]
        source: serde_json::Error,
    },
}

/// The crate's result alias.
pub type Result<T> = std::result::Result<T, Error>;
