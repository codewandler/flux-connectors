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

mod names;
mod op;
mod types;

pub use op::emit_operation;

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

    /// A body field whose name is a **JSON path** rather than a key at the root of the body.
    ///
    /// babelforce's `presence.name` and Zendesk's `ticket.comment.body` both nest
    /// (`providers/babelforce.toml`, `providers/zendesk.toml`), but [`connector_spec::ParamSet`]'s
    /// `body` is a flat `Vec<Param>` carrying one `name` — there is no field recording the path a
    /// body field occupies. Emitting the dotted name as a literal JSON key produces
    /// `{"presence.name": …}`, which is a request the vendor accepts and ignores: the worst
    /// possible failure, because it succeeds.
    ///
    /// So it is refused. Closing this needs an additive field on `Param` — see the crate docs.
    #[error(
        "operation `{operation}`: body field `{name}` names a nested JSON path, and the IR cannot \
         express one — `ParamSet::body` is a flat field list, so this would be emitted as the \
         literal key `\"{name}\"` and silently ignored by the vendor. `Param` needs an additive \
         wire-path field before this operation can be emitted"
    )]
    NestedBodyField {
        /// The operation id.
        operation: String,
        /// The dotted body field name.
        name: String,
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
