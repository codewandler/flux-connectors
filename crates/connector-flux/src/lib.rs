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
//! - [`emit_graph`] — one IR [`connector_spec::Graph`] becomes one formatted **composite** `op`
//!   that calls the connector's own operations. It owns symbol generation and region nesting, and
//!   it refuses every shape whose only other outcome is Flux that is wrong at runtime.
//! - `names` — the wire-name/symbol-name mapping. A vendor may call a parameter `time.start`; Flux
//!   may not, and the two names are kept apart rather than reconciled by mangling.
//! - `types` — JSON Schema to Flux `TypeRef`, **including the documented `Any` fallback** for the
//!   shapes Flux cannot express.
//! - [`highlight`] — the other direction: formatted Flux rendered as a syntax-highlighted SVG,
//!   coloured by flux-lang's own CST classifier (C-45). It lives beside the emitter because both
//!   sides of "what Flux looks like" must come from flux-lang and never from a local imitation of
//!   its grammar.

mod graph;
pub mod highlight;
mod names;
mod op;
mod types;

pub use graph::emit_graph;
pub use op::{emit_operation, parameter_symbols};

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

    /// A **header parameter** pinned with a JSON Schema `const`.
    ///
    /// `const` reads as "this value and no other", and for a *body* field that is exactly what
    /// [`op::constant`](crate::op) honours: the field is sent and never declared. A header parameter
    /// is not the same declaration. `ParamSet::header` means *the caller supplies this*, so honouring
    /// the keyword there would make one list mean two things depending on a schema keyword, and the
    /// IR would no longer say which headers a caller may set.
    ///
    /// Refused rather than emitted, because emitting is what it used to do and the result was the
    /// worst of the three: the parameter stayed in the signature as a required, caller-overridable
    /// argument with the constraint dropped, so a connector that looked pinned sent whatever the
    /// caller passed (C-52's finding, measured again by C-107 against Notion's mandatory
    /// `Notion-Version`). The fix is one table.
    #[error(
        "operation `{operation}`: header parameter `{name}` is pinned with a JSON Schema `const`, \
         but `params.header` means the caller supplies the value — the pin would be dropped and the \
         header would stay a required, caller-overridable argument. A vendor-fixed header is \
         declared in `const_headers`, at provider level or on the operation, and is emitted as a \
         literal"
    )]
    ConstantHeaderParam {
        /// The operation id.
        operation: String,
        /// The pinned header parameter's caller-facing name.
        name: String,
    },

    /// A constant header whose name is not an HTTP field name.
    ///
    /// `http.request` builds a `HeaderName` from it and errors on anything that is not an RFC 9110
    /// token (`../flux/crates/flux-web/src/http.rs:172-174`), so this turns a request that could
    /// never be sent into a build failure — the same check `params.header` entries already get.
    #[error(
        "operation `{operation}`: constant header `{name}` is not an HTTP field name — only ASCII \
         token characters are allowed (RFC 9110 §5.1)"
    )]
    BadHeaderName {
        /// The operation id.
        operation: String,
        /// The unspellable header name.
        name: String,
    },

    /// One header name declared twice on one request: as a constant and as something else.
    ///
    /// The request's header record has one slot per name, so the second declaration overwrites the
    /// first — silently, and in an order nothing in the provider file makes visible. Both spellings
    /// of the collision are real: a `const_headers` entry against a caller-supplied `params.header`,
    /// and either of those against `content-type`, which the emitter derives from the request body.
    #[error(
        "operation `{operation}`: header `{name}` is declared as {first} and as {second}. A request \
         carries one value per header name, so one of the two would be dropped without a word — \
         declare it on one side only"
    )]
    HeaderConflict {
        /// The operation id.
        operation: String,
        /// The contested header name, in the spelling that reaches the wire.
        name: String,
        /// Where the header comes from first.
        first: &'static str,
        /// Where it comes from second.
        second: &'static str,
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

    /// A body field that a `form` encoding cannot carry — C-144.
    ///
    /// `application/x-www-form-urlencoded` is a flat list of `key=value` pairs and **has no agreed
    /// nesting convention**: Stripe writes `metadata[key]`, PHP and Rails write `a[b]` and `a[b][]`,
    /// and an OAuth2 token grant nests nothing at all. Picking one would send a vendor a key it does
    /// not recognise, and a vendor that does not recognise a form key answers `200` and ignores it.
    /// So a declared nesting — a dotted `wire` path, or a field whose declared value is itself an
    /// object or an array — is refused rather than flattened.
    #[error(
        "operation `{operation}`: body field `{name}` (`{wire}` on the wire) cannot be form-encoded \
         — {reason}. `application/x-www-form-urlencoded` is a flat list of `key=value` pairs, and a \
         key a vendor does not recognise is accepted and ignored. Send this body as `json`, or \
         declare the flat spelling the vendor documents"
    )]
    UnencodableFormField {
        /// The operation id.
        operation: String,
        /// The caller-facing field name.
        name: String,
        /// The spelling the vendor would have seen.
        wire: String,
        /// Which property of the field makes it unencodable.
        reason: &'static str,
    },

    /// A free-form `body_schema` under a non-JSON encoding — C-144.
    ///
    /// A free-form body names no fields, so there is no set of pairs for the emitter to assemble. The
    /// JSON path works because flux canonicalizes a whole record with `parse($body, as: "json")`;
    /// flux has **no equivalent for a form body** — `as_type` is restricted to
    /// `f64`/`i64`/`bool`/`json`/`string` by flux-lang's own analyzer — so emitting one would be this
    /// emitter claiming an encoding it cannot produce.
    #[error(
        "operation `{operation}`: a free-form `params.body_schema` cannot be encoded as \
         `{encoding}`. The caller's keys are not known until runtime and flux has no form encoder, \
         so there is nothing to assemble. Declare the body as named `params.body` fields, or send it \
         as `json`"
    )]
    UnencodableFormBody {
        /// The operation id.
        operation: String,
        /// The encoding the operation declared, as the provider file spells it.
        encoding: &'static str,
    },

    /// A `body_encoding` on an operation that sends no body at all — C-144.
    ///
    /// The declaration would change nothing: with no body there is no payload and no `content-type`,
    /// so the encoding is a statement about a request position the operation does not use. That is
    /// the same silent no-op a `const` on a header parameter used to be (see
    /// [`ConstantHeaderParam`](Self::ConstantHeaderParam)), and it is refused for the same reason —
    /// an author who declared it believes something is happening.
    #[error(
        "operation `{operation}`: declares `body_encoding = \"{encoding}\"` but sends no body, so \
         the declaration encodes nothing. Remove it, or declare the `params.body` fields it was \
         meant to encode"
    )]
    BodyEncodingWithoutBody {
        /// The operation id.
        operation: String,
        /// The encoding the operation declared, as the provider file spells it.
        encoding: &'static str,
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

    /// The graph name cannot be spelled as a Flux composite-op **declaration** name.
    ///
    /// The same grammar, and the same refusal, as [`Error::UnspellableOperationId`] — a graph's name
    /// becomes an `op` declaration exactly as an operation id does.
    #[error(
        "graph `{graph}`: a Flux `op` declaration name may only contain ASCII letters, digits, `_` \
         and `-` — a dotted name cannot be declared (see C-23, the operation naming contract)"
    )]
    UnspellableGraphName {
        /// The graph name as the IR carries it.
        graph: String,
    },

    /// The graph has a cycle, so it has no lowering at all.
    ///
    /// Flux has no `goto` and its control flow is strictly nested. An iteration is a bounded loop
    /// node, never an edge pointing backwards, so there is nothing to guess at here.
    #[error(
        "graph `{graph}`: has a cycle{whereabouts}. Flux has no `goto` and its control flow is \
         strictly nested, so a cyclic graph has no lowering at all — an iteration is a bounded loop \
         node, not an edge pointing backwards"
    )]
    GraphCycle {
        /// The graph name.
        graph: String,
        /// Where the cycle was found, when it can be narrowed to one region.
        whereabouts: String,
    },

    /// A value leaves a region through something other than a port the region declares.
    ///
    /// A value may *enter* a region freely — an inner statement reading an outer symbol is fine —
    /// but the escaping direction is where a symbol might not be bound when the block closes. A
    /// region's output ports are the phi node Flux does not have, made explicit.
    #[error(
        "graph `{graph}`: node `{node}` sends `{port}` out of region `{region}`, which declares no \
         output port `{port}`. A value leaves a region only through a port the region declares — \
         otherwise the symbol it lowers to may not be bound when the block closes"
    )]
    RegionCrossingEdge {
        /// The graph name.
        graph: String,
        /// The node the value comes from.
        node: String,
        /// The port it leaves on.
        port: String,
        /// The region it escapes.
        region: String,
    },

    /// A region declares an output port that nothing inside it binds.
    ///
    /// The declaration is a promise the body keeps. Nothing inside binding it means the symbol the
    /// port lowers to is unbound wherever it is read — a runtime failure long after the build
    /// passed, which is the class of defect this pipeline refuses everywhere else.
    #[error(
        "graph `{graph}`: region `{region}` declares output port `{port}`, which no node inside it \
         binds. Every arm of a region must bind every output port it declares — that is the phi \
         node Flux does not have, made explicit"
    )]
    UnboundRegionOutput {
        /// The graph name.
        graph: String,
        /// The region node.
        region: String,
        /// The port with no producer.
        port: String,
    },

    /// Two nodes inside one region both claim its output port, so which value escapes is undecided.
    #[error(
        "graph `{graph}`: region `{region}` declares output port `{port}`, and both `{first}` and \
         `{second}` inside it bind it. One value leaves a region through one port; nothing states \
         which of the two it is"
    )]
    AmbiguousRegionOutput {
        /// The graph name.
        graph: String,
        /// The region node.
        region: String,
        /// The contested port.
        port: String,
        /// The first node claiming it.
        first: String,
        /// The second node claiming it.
        second: String,
    },

    /// A gate declares an output port.
    ///
    /// **A gate exports nothing.** It lowers to Flux's `when`, which has no else branch here, so a
    /// symbol bound inside it is *unbound* on the false path and reading it afterwards fails at
    /// runtime. Exporting a value out of a conditional needs a branch with a default (C-98's
    /// `match`).
    #[error(
        "graph `{graph}`: node `{node}` is a gate declaring output port `{port}`. A gate lowers to \
         Flux's `when`, which has no else branch here — a symbol bound inside it is unbound when \
         the condition is false, and reading it afterwards fails at runtime"
    )]
    GateExportsAValue {
        /// The graph name.
        graph: String,
        /// The gate node.
        node: String,
        /// The port it tried to declare.
        port: String,
    },

    /// A node carries a duration, and **flux-lang 0.39's two formatters disagree about how to spell
    /// one**.
    ///
    /// `flux_lang::format` (the AST printer this crate emits through) writes the suffixed form —
    /// `fmt_duration` turns 60000 into `1m`, 1000 into `1s`, 250 into `250ms`, and never produces a
    /// bare number. `flux_lang::format_cst::format_module` — the formatter a human editing the
    /// generated file actually runs — declines to re-print the suffixed form and returns `None`,
    /// accepting only bare milliseconds (`delay 500`, `per 1000`).
    ///
    /// Both spellings parse to the same AST, so nothing here is ambiguous: this is an **upstream
    /// defect**, not a shape this repository has to decide about. It bites every `throttle` (no
    /// window value avoids the suffix) and every `retry` that declares a delay; a `retry` without one
    /// is unaffected and still lowers.
    ///
    /// Refused rather than emitted because the alternative is shipping a generated module flux's own
    /// formatter cannot format — and rewriting the token afterwards would be exactly the string
    /// surgery on generated Flux that AGENTS.md forbids. It lifts on a flux-lang release whose two
    /// formatters agree.
    #[error(
        "graph `{graph}`: node `{node}` declares `{clause}` of {ms}ms, which flux-lang cannot spell \
         consistently: its AST formatter writes `{clause} {suffixed}` while its CST formatter — the \
         one a human editing the generated file runs — accepts only bare milliseconds and declines \
         to re-print the suffixed form. Both parse to the same AST, so this is an upstream defect \
         rather than an ambiguity here. Emitting it would ship a module flux's own formatter cannot \
         format. A `retry` without a delay is unaffected"
    )]
    UnspellableDuration {
        /// The graph name.
        graph: String,
        /// The node id.
        node: String,
        /// The clause carrying the duration (`delay` or `per`).
        clause: &'static str,
        /// The duration in milliseconds, as the IR carries it.
        ms: u64,
        /// The suffixed spelling flux's AST formatter would write.
        suffixed: String,
    },

    /// A `select` node reads a path out of an **operation's** response.
    ///
    /// **This is the blocker the flow-graph design states first.** `http.request` returns one flat
    /// string — `format!("HTTP {status}\n{headers}\n{body}")`
    /// (`../flux/crates/flux-web/src/http.rs:221-225`) — not a record. Flux's `jq` parses a *whole*
    /// string as JSON before extracting, so a path applied to that string resolves to `null` on
    /// every response, success or failure. Splitting the body back out needs an `expr` escape, and
    /// an escape inside a formula is not a fixed point of flux's own formatter, so the emitted
    /// module stops round-tripping.
    ///
    /// So this is a **refusal, not a degradation**: emitting a selector that always yields null is
    /// precisely the plausible-but-wrong output AGENTS.md forbids. It lifts when `http.request`
    /// returns a record, which is a seam story on flux.
    #[error(
        "graph `{graph}`: node `{select}` selects `{path}` from node `{from}`, which calls \
         operation `{operation}`. `http.request` returns one flat string — `HTTP \
         {{status}}\\n{{headers}}\\n{{body}}` — not a record, so a path applied to it resolves to \
         `null` on every response, success or failure. Splitting it needs an `expr` escape that is \
         not a fixed point of flux's formatter. Return the response whole and select from it once \
         `http.request` returns a record"
    )]
    SelectOnAnOperation {
        /// The graph name.
        graph: String,
        /// The `select` node.
        select: String,
        /// The path it tried to read.
        path: String,
        /// The node producing the response.
        from: String,
        /// The operation that node calls.
        operation: String,
    },

    /// A `retry` region declares more than one output port.
    ///
    /// Flux's `retry` binds **one** result — its `-> $bind` names the body's final value. Two
    /// declared ports would need two binds, and choosing one silently would drop the other.
    #[error(
        "graph `{graph}`: region `{region}` is a retry declaring {count} output ports. Flux's \
         `retry` binds one result (`-> $symbol`), so a retry exports at most one value"
    )]
    RetryExportsMoreThanOneValue {
        /// The graph name.
        graph: String,
        /// The retry node.
        region: String,
        /// How many ports it declared.
        count: usize,
    },

    /// A node this lowering cannot spell, with the reason stated in full.
    ///
    /// Every case is a shape whose only alternatives are a refusal or emitted Flux that is wrong in
    /// a way a vendor answers `200` to: an argument the operation does not declare, a template
    /// placeholder naming no port, a record with no fields, a composite literal that flux's own
    /// formatter would re-space.
    #[error("graph `{graph}`: node `{node}` cannot be lowered — {reason}")]
    UnlowerableGraphNode {
        /// The graph name.
        graph: String,
        /// The node id.
        node: String,
        /// Why it cannot be lowered.
        reason: String,
    },

    /// The lowering produced text that is not canonical Flux — the C-11 gate, applied to this
    /// crate's own output.
    ///
    /// Unreachable by construction, and loud on purpose: reaching it means a node shape got past the
    /// refusals above and produced a module flux would reformat, fail to parse, or decline to
    /// publish an op from. Emitting it anyway would ship an artifact nobody can review a diff of.
    #[error("graph `{graph}`: the lowering produced Flux that is not canonical — {reason}")]
    GraphNotCanonical {
        /// The graph name.
        graph: String,
        /// What the gate found.
        reason: String,
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
