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
//!   [`emit_graph_with_paths`] returns the [`NodePaths`] map beside the module — node id → the
//!   flux node path (`body[3].then[0]`) of the statement that node produced — so an analyzer
//!   finding about generated Flux can be shown on the node an author drew (C-96).
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

pub use graph::{emit_graph, emit_graph_with_paths, NodePaths};
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
    /// A bracket is the same ambiguity one story later (C-185): `items[0]` is either a field
    /// literally called `items[0]` or element zero of `items`, and only a `wire` can say which.
    ///
    /// So it is refused, and the fix is one line in the provider file.
    #[error(
        "operation `{operation}`: body field `{name}` has a structural name — a `.` or a `[` — and \
         no `wire`, so whether it is one field called `\"{name}\"` or a path into the body is \
         undecidable. A body field's JSON path is declared with `wire` — write `wire = \"{name}\"` \
         and give the field a caller-facing `name`. Emitted as-is it would be the literal key \
         `\"{name}\"`, which the vendor accepts and ignores"
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

    /// An operation whose emitted text flux's own CST formatter cannot re-print — C-185.
    ///
    /// The emitter prints an AST and then hands the result to `format_cst::format_module`, whose
    /// spelling is the one the canonicality gate compares against; see
    /// [`op::emit_operation`](crate::emit_operation) for why. That formatter declines — returns
    /// `None` — when its equivalence guard cannot prove the re-print lowers to the same module, and
    /// it is the guard that makes the round trip safe. A refusal here therefore means flux cannot
    /// spell this operation at all, which is a shipped module a human's formatter would rewrite:
    /// the same class of upstream defect [`Error::UnspellableDuration`] records, and it is refused
    /// for the same reason.
    #[error("operation `{operation}`: {reason}")]
    NotCanonical {
        /// The operation id.
        operation: String,
        /// What the formatter said, or could not say.
        reason: String,
    },

    /// A `wire` path whose array index is not one — C-185.
    ///
    /// `items[0]` is the only spelling an index has here. Everything else a bracket can be part of
    /// is refused rather than absorbed into an object key, because absorbing it produces
    /// `{"items[": …}` — a key the vendor accepts and ignores, which is the failure mode every
    /// refusal in this enum exists to avoid.
    #[error(
        "operation `{operation}`: body field `{name}` declares `wire = \"{wire}\"`, whose segment \
         `{segment}` is not an array index — {reason}. An element of an array is spelled \
         `key[0]`, one index per path segment"
    )]
    BadArrayIndex {
        /// The operation id.
        operation: String,
        /// The caller-facing field name.
        name: String,
        /// The whole path.
        wire: String,
        /// The segment that could not be read.
        segment: String,
        /// What is wrong with it.
        reason: &'static str,
    },

    /// A `wire` segment that is all digits, with no brackets to say whether it is an array index —
    /// C-185.
    ///
    /// Before an array had a spelling, `items.0.value` built `{"items": {"0": {"value": …}}}`, and
    /// `providers/sendgrid.toml` records an author reading that path as an array and getting an
    /// object. Now that `items[0].value` means the array, the two spellings sit one character apart
    /// and mean different requests, so the ambiguous one is refused rather than left as a trap.
    ///
    /// The cost is that a vendor whose object key is genuinely a number is unspellable. No provider
    /// in this repository has one (measured 2026-08-02:
    /// `grep -n 'wire = "[^"]*\.[0-9]' providers/` returns nothing), and this refusal is where that
    /// case gets reconsidered when one arrives.
    #[error(
        "operation `{operation}`: body field `{name}` declares `wire = \"{wire}\"`, whose segment \
         `{segment}` is all digits — an array index is spelled with brackets, so write \
         `…{segment_bracketed}…` for element {segment} of the array before it. A numeric object \
         *key* has no spelling here"
    )]
    NumericWirePathSegment {
        /// The operation id.
        operation: String,
        /// The caller-facing field name.
        name: String,
        /// The whole path.
        wire: String,
        /// The all-digit segment.
        segment: String,
        /// The same index in the spelling that builds an array, so the message can show it.
        segment_bracketed: String,
    },

    /// An array declared with a hole in it — C-185.
    ///
    /// An array's length is the set of indices its fields declare, so `items[0]` and `items[2]`
    /// with no `items[1]` has no honest emission. Closing the gap sends the third element in the
    /// second position; filling it with `null` sends a value the author never wrote. Both are
    /// requests a vendor answers.
    #[error(
        "operation `{operation}`: the body array `{path}` is declared at {declared} but not at \
         [{missing}] — an array's elements are its declared indices, and a hole in them would \
         either shift an element into another's position or send a `null` nobody wrote"
    )]
    SparseBodyArray {
        /// The operation id.
        operation: String,
        /// The path of the array, in `wire` spelling.
        path: String,
        /// The lowest index no field declares.
        missing: usize,
        /// The indices that are declared, rendered as `[0], [2]`.
        declared: String,
    },

    /// Two body fields whose wire paths cannot both exist: one occupies a JSON path that the other
    /// needs as an object or an array, or two fields claim the same path outright.
    ///
    /// `ticket.comment` and `ticket.comment.body` are not composable — the first says
    /// `comment` holds the caller's value, the second says it holds an object. Emitting either
    /// silently discards the other, so the operation is refused instead of one field being dropped.
    /// `items[0].value` and `items.value` are the same conflict with an array on one side (C-185).
    #[error(
        "operation `{operation}`: body fields `{first}` and `{second}` claim conflicting wire \
         paths — `{path}` cannot be a value, an object and an array at once. One of the two would \
         be silently dropped from the request"
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

    /// **A request slot claimed by an operator's pin and by a caller's parameter at once** — C-187.
    ///
    /// A `[[config]]` field binding `path.zone_id` or `query.teamId` says *this connector is
    /// installed for this tenant*. An operation declaring the same path segment or query parameter
    /// says *the caller chooses the tenant on every call*. There is no reading under which both are
    /// true, and neither precedence is safe to pick silently: honour the parameter and the pin is
    /// decoration, honour the pin and the operation declares an argument whose value vanishes. Both
    /// produce a request the vendor answers `200` to, addressed to a tenant nobody chose.
    ///
    /// The same judgement as [`ConstantHeaderParam`](Self::ConstantHeaderParam) one story earlier,
    /// applied to the two positions a pin reaches that a constant header does not. A pinned *header*
    /// colliding with anything is an [`Error::HeaderConflict`] instead, which already compares every
    /// source that can claim a header name.
    #[error(
        "operation `{operation}`: the {position} value `{name}` is both pinned by a `[[config]]` \
         field and declared as a parameter of this operation. A value an operator pins at install \
         time and a caller may also pass is not pinned — declare it on one side only"
    )]
    PinnedValueConflict {
        /// The operation id.
        operation: String,
        /// The request position, as `binds` spells it — `path` or `query`.
        position: &'static str,
        /// The contested name.
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

    /// A query value whose wire representation is not a JSON scalar — C-30.
    ///
    /// Flux 0.54 encodes strings, numbers and booleans through `http.request(query: ...)`, but
    /// arrays and objects require a vendor-specific convention (repeated keys, delimiters,
    /// brackets or JSON). `Any` is refused with them because it could carry either at runtime.
    #[error(
        "C-30: operation `{operation}` query parameter `{name}` has type `{kind}`, which cannot be \
         encoded without choosing a vendor-specific collection convention. Declare a scalar \
         schema, or withhold the operation until its query serialization is modelled"
    )]
    UnencodableQueryValue {
        /// The operation id.
        operation: String,
        /// The caller-facing parameter name.
        name: String,
        /// The Flux type derived from its schema.
        kind: String,
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

    /// **An operation that mints a credential cannot be emitted as Flux** — C-136.
    ///
    /// The diversion this repository ships is a **host** mechanism: `connector_pack::mint` takes the
    /// secret out of the transport's answer, writes it through the bound `CredentialStore`, and
    /// hands the caller `{ "credential": "tenants/…" }`. Nothing on that path is expressible here.
    /// An emitted `op` binds `response = http.request(…)` and returns it — so a module carrying a
    /// login would perform the login and bind the **raw token to a model-visible symbol**, while the
    /// same operation's published `response_schema` promised a handle. Two artifacts disagreeing,
    /// with the executable one wrong.
    ///
    /// `AGENTS.md` § Authentication contract states the invariant this enforces, and it is not a
    /// consequence of the diversion but older than it: *"Generated Flux names a credential and
    /// nothing more. It must not add prefixes, base64-encode pairs, refresh tokens, or perform
    /// session login. … Putting acquisition in Flux would expose raw tokens in model-visible
    /// symbols."*
    ///
    /// # Why this is a refusal and not an exclusion
    ///
    /// "Emit it into the catalogue but not into the module" is the tempting answer and it is not
    /// available: `emit_operation` produces **one** rendering, and `connector-cli`'s seam feeds that
    /// same text to the module, to the per-operation `.flux` file and to `web/public/catalog.json`
    /// — deliberately, so that module-and-catalogue agreement is a property rather than a
    /// coincidence. Splitting it would put a login's Flux in the public catalogue anyway, and would
    /// break the three coherence checks that currently make the split unnecessary.
    ///
    /// So the honest statement is the one this variant makes: **this repository's execution format
    /// cannot express a credential-producing operation.** The declaration, the derived handle output
    /// and the host-side diversion all exist; what does not exist is a way to emit one, and until
    /// there is, declaring one does not build. See `docs/stories/C-136-credential-diversion.md`
    /// § Open question — the resolution may well be that the trigger belongs on `OAuth2Spec` and
    /// that no such operation should exist at all.
    #[error(
        "operation `{operation}`: it declares `produces_credential` (minting `{credential}` from \
         `{secret}`), and this repository's execution format cannot express that. An emitted `op` \
         binds `response = http.request(…)` and returns it, so the module would perform the login \
         and put the raw token in a model-visible symbol — `AGENTS.md` § Authentication contract: \
         generated Flux \"must not … perform session login\". The diversion is a host mechanism \
         (`connector_pack::mint`) and the module cannot reach the credential store. Withhold the \
         operation until C-136's open question is settled; `expose = false` is not the mechanism, \
         since an unexposed operation is still emitted (C-413)"
    )]
    CredentialProducingOperation {
        /// The operation id.
        operation: String,
        /// The credential it declares it mints.
        credential: String,
        /// The response location the secret was to be diverted from.
        secret: String,
    },

    /// An authored write declared the risk of a read.
    ///
    /// flux's approval gate reads `risk`, and `low` is the tier that passes without a human. A
    /// A connector-authored `write` changes state the vendor owns, which is not something this
    /// emitter can certify as unsurprising — [`connector_spec::Risk::Low`] is documented as "reads,
    /// and writes that cannot surprise anyone". Refused rather than quietly raised: a silent
    /// correction would hide the authoring mistake that produced it, and the IR omits `Default` on
    /// all three fields precisely so none is decided by silence or HTTP method.
    #[error(
        "operation `{operation}`: a {direction} changes state the vendor owns and may not declare \
         `risk = \"low\"` — flux's approval gate waves `low` through without a human. Declare the \
         risk this write actually carries"
    )]
    WriteDeclaredLowRisk {
        /// The operation id.
        operation: String,
        /// The connector-authored operation direction.
        direction: &'static str,
    },

    /// An authored write declared itself idempotent.
    ///
    /// HTTP replay semantics do not answer whether Flux may reuse a cached result instead of
    /// executing a vendor mutation. **This refusal is unconditional.**
    /// The escape for a write that really is replay-safe is `idempotency = "conditional"` plus a
    /// stated `repeatable_because`, which is the value `flux_spec::coherence` reserves for exactly
    /// this: `Idempotent` licenses flux's op cache to serve a stored result *instead of executing*,
    /// which is a stronger claim than "repeating is safe" and not one a cache purge can make.
    #[error(
        "operation `{operation}`: a {direction} may \
         not declare `idempotency = \"idempotent\"` — flux would treat a retry around it as safe, \
         and its op cache would be licensed to serve a stored result instead of running the call at \
         all. If repeating this endpoint really is safe, declare `idempotency = \"conditional\"` \
         and state the condition in `repeatable_because`; otherwise use `non_idempotent`"
    )]
    WriteDeclaredIdempotent {
        /// The operation id.
        operation: String,
        /// The connector-authored operation direction.
        direction: &'static str,
    },

    /// A mutating operation declared `conditional` without stating the condition.
    ///
    /// flux's I3 names `Conditional` as the escape hatch for a write that is "genuinely safe to
    /// repeat under **stated** conditions". Before C-186 nothing made anyone state them, and six
    /// shipped operations — three of them Stripe money movements — declared `conditional` with the
    /// condition in no field and no artifact. A host read that some condition existed and learned
    /// nothing about what it was.
    #[error(
        "operation `{operation}`: a {direction} declares `idempotency = \"conditional\"` but states no \
         `repeatable_because`. `conditional` is the claim that repeating is safe *under a stated \
         condition*; say what makes it safe — what the vendor does on a repeat — so a reviewer and a \
         host can both read it"
    )]
    ConditionalWithoutItsCondition {
        /// The operation id.
        operation: String,
        /// The connector-authored operation direction.
        direction: &'static str,
    },

    /// `repeatable_because` was stated on an operation that changes nothing.
    ///
    /// The field's whole meaning is "here is why repeating this *write* is safe". On a `GET`, `HEAD`
    /// or `OPTIONS` there is no repeat hazard to condition. Left permitted, it would be copied onto
    /// operations that never needed it until no reviewer read any of them, which is how an escape
    /// hatch quietly becomes the norm.
    #[error(
        "operation `{operation}`: `repeatable_because` is stated on a {direction}, which changes \
         nothing and so has no repeat hazard to put a condition on. The field exists only to state \
         the condition behind `idempotency = \"conditional\"` on a write; remove it"
    )]
    RepeatabilityConditionUnneeded {
        /// The operation id.
        operation: String,
        /// The connector-authored operation direction.
        direction: &'static str,
    },

    /// `repeatable_because` was stated beside an `idempotency` that is not `conditional`.
    ///
    /// The condition describes a repeat that is safe while the field it sits next to says otherwise.
    /// One of the two is wrong, and shipping both is C-186's own defect — prose and field
    /// disagreeing about one fact — arriving from the other direction.
    #[error(
        "operation `{operation}`: `repeatable_because` is stated but `idempotency` is \
         `{idempotency}`, not `conditional`. The condition says repeating is safe and the field says \
         it is not; fix whichever one is wrong rather than shipping both"
    )]
    RepeatabilityConditionWithoutTheClaim {
        /// The operation id.
        operation: String,
        /// The declared idempotency, as a provider file spells it.
        idempotency: &'static str,
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
