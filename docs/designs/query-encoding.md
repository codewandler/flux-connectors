# Design: percent-encoding for generated query values

> Parent story: [C-28](../stories/C-28-query-percent-encoding.md) · Found by
> [C-8](../stories/C-8-flux-op-emitter.md) · Companion handoff:
> [query-encoding-flux-stories.md](query-encoding-flux-stories.md)

**Bottom line, stated first because C-17 is authoring the zendesk provider right now:**
`zendesk.ticket.search` is **not** emittable correctly today and must be **refused**, not emitted.
The gap C-8 reported is real, and it is worse than "spaces break": a caller-supplied query value can
**inject additional query parameters** into the request. The fix belongs in flux. This document
specifies it, and specifies what this repo does in the meantime.

### Provenance of the citations

flux was read **read-only** at tag `v0.38.0` (`git describe --tags` → `v0.38.0`, worktree clean).
Line numbers describe that tree; re-grep by symbol if one does not land. Claims about `url` crate
behaviour were **executed**, not recalled — against `url` 2.5.x, the version flux pins
(`../flux/Cargo.lock:6930-6932`). The probe program is reproduced in §2.2 so the result is
re-runnable.

One citation from an earlier story could **not** be re-verified and is flagged rather than repeated:
[provider-operation-inventory.md](provider-operation-inventory.md) §3.3.5 cites
`../flux/plugins/zendesk/src/main.rs:426-441` for Zendesk's strict `%20`-never-`+` encoding. At
`v0.38.0` that file does not exist — `plugins/zendesk/` is absent from disk and from
`git ls-files plugins/` (only a stale build artifact, `plugins/target/debug/flux-plugin-zendesk`,
remains). The zendesk plugin was never a tracked file in the flux repository. The conclusions below
**do not depend on that citation**: the encoding requirement is derived from RFC 3986 and from the
executed `url` probe instead. See §5.3.

---

## Why

### 1. What the emitter produces today

`connector-flux` assembles the query string by string interpolation
(`crates/connector-flux/src/op.rs:192-233`). A required parameter is concatenated into the `fmt`
template; an optional one is appended inside a `when` guard:

```flux
$sep = "?"
when $query
  $url = fmt("{url}{sep}query={query}")
```

Nothing between the caller's value and the wire encodes anything. `fmt` interpolates verbatim.
C-8 recorded this honestly rather than half-fixing it
(`crates/connector-flux/src/op.rs:58-63`), and that judgement is upheld here.

### 2. The gap in flux is real — the whole catalog was walked

**No registered op percent-encodes.** Three independent checks agree:

| Surface | Evidence |
|---|---|
| Documented catalog | `../flux/crates/flux-flow/docs/ops-reference.md:15-88` — the full quick-reference table. `html_to_markdown` is the only pure string transform; there is no encoder. |
| Actual `ToolSpec` registrations | A census of every `name: "…"` literal in `flux-tools/src/`, `flux-web/src/` and `flux-flow/src/` yields 80 op names (incl. test fixtures). None encodes. Sweeping **all** of `crates/` for an op name matching `encode\|escape\|url\|quote\|uri` returns **nothing**. |
| `expr` built-ins | `is_known_expr_fn` (`../flux/crates/flux-lang/src/expr.rs:804-828`) whitelists exactly 21 names: `round abs min max len lower upper trim reverse contains replace repeat concat sum any all has join split first last`. No encoder. The diagnostic string at `:139-143` repeats the same list, so the whitelist is closed by construction. |

**Verdict: the gap is real. C-8 missed nothing.**

What flux *does* have is the encoder itself, written **four separate times**, all private:

- `percent_encode_component` — `../flux/crates/flux-plugin/src/host.rs:1843-1854`
- `percent_encode_segment` — `../flux/crates/flux-providers/src/bedrock.rs:552-562`
- `urlencode` — `../flux/crates/flux-credentials/src/lib.rs:1435-1446`
- `urlencode` — `../flux/plugins/jira/src/operations/mod.rs:857`

The first three are byte-identical: preserve `A-Za-z0-9-._~`, `%XX` uppercase for everything else.
So flux has settled on an encoding; it has just never exposed it above the Rust boundary. **A fourth
copy is not needed. A shared home is.**

### 3. It is not "spaces break" — it is silent parameter injection

The failure surface is narrower *and* sharper than C-8's note assumed, because `http.request` parses
the interpolated URL with `url::Url::parse`
(`../flux/crates/flux-system/src/net.rs:126`, inside `guard_and_pin`; called from
`../flux/crates/flux-web/src/http.rs:157`) and that parse silently repairs *some* characters.
Executed results (§2.2 for the program):

| Input value | Result | Verdict |
|---|---|---|
| `type:ticket status:new` | `query=type:ticket%20status:new` | **works** — space rescued by the parser |
| `type:ticket created>2024-01-01` | `query=type:ticket%20created%3E2024-01-01` | **works** |
| `billing & invoicing` | pairs = `[("query","billing "), (" invoicing","")]` | **corrupt** — value truncated, extra param invented |
| `refund for ticket #4521` | query = `query=refund%20for%20ticket%20`, fragment = `4521` | **corrupt** — tail never leaves the process |
| `a+b` | pairs = `[("query","a b")]` | **corrupt** — `+` silently becomes a space |
| `a\nb` | query = `query=ab` | **corrupt** — newline silently deleted |
| `x&per_page=1&admin=true` | pairs = `[("query","x"),("per_page","1"),("admin","true")]` | **injection** |

Three things follow, and each changes a decision downstream.

**(a) The obvious smoke test passes.** `type:ticket status:new` — the exact example in C-28's own
Notes — *works today*, by accident, because the `url` crate percent-encodes space in the query
component. Anyone spot-checking this gap with the canonical Zendesk expression will conclude it is
fixed. It is not. This is precisely the "looks correct and is wrong" hazard C-8 named, arriving from
an unexpected direction.

**(b) The last row is a safety finding, not a correctness bug.** `http.request` is a model-visible
tool and the connector's query values are model-supplied. A value containing
`&per_page=1&admin=true` adds or overrides query parameters on a request the operator authorized for
a different shape. It can raise a page cap, flip a boolean the operator pinned, or — once C-271
lands `AuthScheme::Query` injection — collide with the parameter name carrying a credential. This
promotes the fix from ergonomics to the safety envelope and is the main reason §3 recommends the
form it does.

**(c) `#` loses data with no error.** Everything after `#` becomes a URL fragment, which is never
transmitted. A ticket reference (`"ticket #4521"`) is exactly the shape support agents type.

### 4. Why this blocks a launch operation

`zendesk.ticket.search` (`GET /api/v2/search.json`) takes a required `query` parameter carrying a
free-form Zendesk search expression ([provider-operation-inventory.md](provider-operation-inventory.md)
§3.2 row 2, §3.3). Free-form means arbitrary agent- or user-derived text, which means `&`, `#` and
`+` are reachable in normal use. It is one of the seven operations the zendesk connector must cover
to replace the plugin (§3.2). See §5 for the verdict.

---

## Approach

### 1. Can this be solved connector-side at all? No.

Three candidates were considered and all three fail. This is a definite answer, not a menu.

**Pre-encoding at generation time — no.** The emitter can encode a *literal* it writes into the
template, but every query value that matters is bound to an `op` parameter and supplied by the
caller at run time. `$query` does not exist until the model calls the op. Encoding at build time
encodes the placeholder `{query}`, not the value. Since the whole point of a generated connector is
that the model supplies the arguments, build-time encoding covers exactly the cases that were never
at risk. It is not a partial fix; it is a fix for the empty set.

**`expr` string functions — no.** `replace` is in the whitelist
(`../flux/crates/flux-lang/src/expr.rs:815`), so `replace(q, " ", "%20")` parses. Composing a
correct RFC 3986 encoder out of it would need one `replace` per character class, in a fixed order
with `%` first, over a character set that includes non-ASCII bytes `expr` has no way to iterate.
There is no `char`/`ord`/`hex` builtin and no loop. It cannot express the transform. More
importantly, an approximation here is the exact thing C-8 refused: it would fix the visible
characters and leave the injection hole open, converting a known gap into an unknown one.

**Composition of existing ops — no.** The census in §2 is exhaustive: the catalog contains no op
that transforms a string into its percent-encoded form, and no pair of ops composes into one.
`html_to_markdown` is the only pure string op and it is not invertible into an encoder.

**Therefore flux must change.** Everything below specifies that change.

### 2. Recommendation: a structured `query` parameter on `http.request`

Two shapes were weighed. The recommendation is the **structured parameter**, not the pure op.

#### The alternative: a pure `urlencode` op

Register a pure, Low-risk, no-effect op alongside `html_to_markdown` — which is the right precedent
(`../flux/crates/flux-web/src/fetch.rs:261-286`: `effects: Vec::new()`, `risk: Risk::Low`,
`group: None`, so it is ungated and always available wherever `http.request` is). The emitter would
then produce:

```flux
$q = call urlencode { value: $query }
$url = fmt("{url}{sep}query={q}")
```

This is a genuinely small change and it does fix correctness. It is nonetheless the weaker option:

- **It is opt-in, and the string-building stays.** Encoding becomes a discipline the caller must
  remember. Correct output depends on every emission path calling it — and on every hand-authored
  flow and every direct model call to `http.request` doing the same. The injection hole in §2.3(b)
  stays open for everyone who does not.
- **It does not remove the `?`/`&` assembly.** `$sep` and the `when` ladder survive, so the emitter
  keeps the most error-prone code it has.
- **It costs a step event per parameter.** Each `call` appends a `StepStarted`/`StepFinished` pair to
  the run event stream. A six-filter operation emits six extra steps whose only content is string
  manipulation, which is noise in every trace and every replay.

#### The recommendation: `query` on `http.request`

```jsonc
"query": {
  "type": "object",
  "description": "Query parameters, appended to `url`. Each key and value is percent-encoded
                  (RFC 3986). A null value is omitted. Prefer this over building a query string
                  into `url`: a value interpolated into `url` is NOT encoded and can inject
                  additional parameters.",
  "additionalProperties": true
}
```

`"required"` stays `["url"]` — this is a purely additive schema change and no existing caller breaks.
The emitted op collapses to:

```flux
$url = fmt("{base}/api/v2/search.json")
do http.request {
  method: "GET",
  url: $url,
  query: { query: $query, per_page: $per_page }
}
```

Why this wins, in order of weight:

1. **It makes the corruption unrepresentable rather than merely avoidable.** The value never enters
   a string that is later parsed as a URL, so there is no encoding step to forget and no injection
   vector to leave open. This is the difference between a fix and a convention.
2. **It closes the hole for the model too,** not just for generated Flux. `http.request` is an
   LLM-visible tool; the structured parameter is the only form that constrains what a model can put
   on the wire.
3. **It deletes the emitter's `$sep`/`when` machinery outright,** and with it the truthiness bug at
   `crates/connector-flux/src/op.rs:54-56` — `when $offset` treats a deliberate `0` as absent, so
   `?offset=0` and `?public=false` are unsendable today. Null-omission in a structured map is
   value-precise where a truthiness guard is not. See §4 and the second question in §6.
4. **C-271 needs this machinery anyway.** The outbound-`$auth` `Query`-scheme story
   ([auth-seam-flux-stories.md](auth-seam-flux-stories.md) F-6) must append a credential parameter to
   the guarded URL, and its Acceptance cites *"`http.request` has no `query` parameter"* as the
   reason it must add a request-level `auth` array instead. Landing `query` first makes C-271
   **smaller**, not larger, and gives both stories one append path instead of two.

The honest cost: `http.request` has a stable contract and this widens it. That cost is real and it
is the reason the pure op was weighed seriously. It is outweighed by points 1 and 2 — a bypassable
fix to a parameter-injection hole is not a fix.

**Recommendation: ship the structured `query` parameter. Do not ship both.** Two ways to spell the
same thing, one of them bypassable, is worse than either alone.

### 3. Encoding semantics — specified, because the obvious implementation is wrong

The obvious implementation is `url::Url::query_pairs_mut().append_pair(k, v)`, and
[auth-seam-flux-stories.md:435-436](auth-seam-flux-stories.md) already recommends exactly that to
C-271's implementor. **For query *values* it is wrong**, and this was executed, not assumed:

```
append_pair("query", "type:ticket status:new")  →  query=type%3Aticket+status%3Anew
```

`append_pair` serializes as `application/x-www-form-urlencoded`, where space becomes `+`.
A server that percent-decodes per RFC 3986 (rather than form-decoding) reads that `+` as a literal
plus and the search expression is silently wrong. The converse does not hold: `%20` is decoded to a
space by **both** decoders — confirmed by probe (`?query=a%20b` → `query_pairs()` yields
`("query", "a b")`). So RFC 3986 encoding is correct under both regimes and form-encoding is correct
under only one.

The specified encoding is therefore flux's own existing one — preserve `A-Za-z0-9-._~`, emit `%XX`
uppercase for every other byte — which is what `percent_encode_component`
(`../flux/crates/flux-plugin/src/host.rs:1843`) and its two twins already do. Promote one to a
shared home; do not write a fifth.

Full semantics the flux story must nail down:

| # | Rule | Rationale |
|---|---|---|
| 1 | Parameters are appended **before** `guard_url_scoped_pinned` (`crates/flux-web/src/http.rs:157`) | The guarded and pinned URL must be the one actually sent. Same ordering constraint C-271 states for `Query` auth. |
| 2 | Keys and values are RFC 3986 percent-encoded (`%20`, never `+`) | §3 above. |
| 3 | A `null` value omits the parameter entirely | This is what makes an optional filter expressible without a truthiness guard. |
| 4 | Number and boolean values use their JSON scalar text, then encode | Avoids forcing the emitter to stringify, which would re-introduce a formatting decision. |
| 5 | Array and object values are a **caller error**, not a guess | Vendor conventions for repeated parameters differ irreconcilably (`a=1&a=2` vs `a=1,2` vs `a[]=1`). Picking one silently is the §2.3 mistake again. When a connector needs one, it becomes an explicit C-12 quirk. |
| 6 | A key already present in `url`'s query string is an **error**, not a silent duplicate | Mirrors C-271's note; a duplicate is exactly how an override sneaks in. |
| 7 | Once C-271 lands: auth parameters apply **after** caller `query` params, and a caller key colliding with an auth parameter name is refused **before the credential is resolved** | Without this, a model-supplied `query: {api_key: "…"}` interacts with credential injection. |

Rule 7 is the item most likely to be dropped if the story is trimmed, and is the one with security
consequences.

### 4. Connector-side behaviour

#### Today

`request_body` (`crates/connector-flux/src/op.rs:186-255`) interpolates every query value verbatim
and emits nothing that encodes. Values reach the wire raw.

#### Once flux ships `query`

The whole query-assembly block — the `?`/`&` template concatenation at `:194-198`, the `$sep` bind at
`:207-210`, and the `when` ladder at `:211-233` — is replaced by one `query` field on the
`http.request` argument object, built as a `Node::Obj` of `Node::Var`s exactly the way `url` and
`method` already are (`:236-253`). Optional parameters are passed as `null` by the caller and
omitted by rule 3, so the guard disappears and the `0`/`false` truthiness bug goes with it.

`$sep`, `SEP` (`:80`) and its `RESERVED` entry (`crates/connector-flux/src/names.rs:22`) are deleted,
not left dormant.

#### In the meantime — what the emitter must **refuse**

**An operation with any query parameter of a string-ish type must be refused, not emitted.**

The refusal is not conservatism. Emitting `query: {…}` against a flux that has not shipped it fails
**silently and totally**, and both halves of that were verified:

- The analyzer **accepts** it. `check_call_args` states it verbatim at
  `../flux/crates/flux-lang/src/analyze.rs:548-549`: *"extra fields are not errors (the runtime/op
  decides)"*. So the module parses and analyzes clean — which is this repo's load-bearing CI gate
  (AGENTS.md).
- The runtime **ignores** it. `HttpRequestTool::execute` reads exactly `url`, `method`, `headers`,
  `body`, `timeout` (`../flux/crates/flux-web/src/http.rs:137-160`) and never looks at `query`.

Net effect of emitting early against an older flux: every filter is dropped, the request returns
`200 OK` with the wrong result set, and nothing anywhere reports a problem. A silent wrong answer is
worse than a build failure, so the emitter refuses until the flux floor is known.

The refusal follows C-8's established pattern — a `connector_flux::Error` variant naming the owning
story (`crates/connector-flux/src/lib.rs:22-72`, `OutOfSlice` / `UnspellableOperationId`):

```rust
/// A query parameter whose value cannot be safely carried today.
///
/// flux has no percent-encoding op and `http.request` has no structured `query` parameter, so a
/// caller-supplied value reaches the wire raw: `&` invents parameters, `#` truncates the request,
/// `+` becomes a space. Emitting anyway is silent corruption — the analyzer accepts an unknown
/// argument and the runtime ignores it — so this is refused until the flux change lands (C-28).
#[error(
    "operation `{operation}`: query parameter `{name}` carries free-form text, which cannot be \
     percent-encoded — flux registers no URL-encoding op and `http.request` has no `query` \
     parameter (see C-28)"
)]
UnencodableQueryValue { operation: String, name: String },
```

**Scope of the refusal — deliberately narrow.** Refusing every query parameter would block the six
zendesk operations that only take numeric ids and page bounds, which are unaffected: a `Number`
value cannot contain `&`, `#`, `+` or a space. So refuse a query parameter whose IR type is
string-ish (`String`, or an untyped/`Any` parameter, which must be treated as string-ish because it
*may* carry text); allow `Number` and `Boolean` through unchanged.

This is a judgement with a live risk attached: it means correctness rests on the IR's type
information being right. An enum or date parameter typed `String` gets refused where it is in fact
safe — an acceptable false positive. A free-form parameter mistyped as `Number` gets emitted where
it is not — the failure this does not catch. See Risks.

The refusal is temporary by construction: it is deleted in the same change that emits `query`.

---

## Alternatives considered

**Half-fix with `expr`'s `replace` for spaces only.** Rejected, and C-8's judgement is upheld
explicitly. §2.3 makes the case stronger than C-8 could: spaces are *already* handled by
`url::Url::parse`, so a space-only `replace` fixes nothing at all while creating the appearance of a
fix. It would also leave the parameter-injection hole open behind a change that looks like it closed
it.

**Emit the encoding as generated Flux.** A composite op that percent-encodes using `expr` builtins.
Rejected: §3.1 shows `expr` cannot express the transform, and even if it could, this violates
AGENTS.md's "no homegrown DSL" — reimplementing a standard string transform in interpolation
primitives is a second little language wearing a disguise.

**Require every connector to be a flux plugin instead.** Every flux plugin that needs this has its
own private `urlencode` (§2, four copies). That is exactly the non-scaling this repo exists to
correct, and it answers a codegen gap by deleting the codegen.

**Pass the query string pre-encoded from the caller.** Declare `query` as "already
percent-encoded, caller's responsibility". Rejected: the caller is an LLM. A contract that is
correct only when the model remembers to encode is not a contract, and it makes the injection vector
a documented feature.

**Ship both the `urlencode` op and the structured parameter.** Rejected — see §3.2. Two spellings,
one bypassable, is a worse contract than either.

---

## Risks & open questions

**The type-based refusal scope depends on IR type fidelity.** §4 refuses string-ish query parameters
and admits numeric ones. A free-form vendor parameter that the IR types as `Number` would be emitted
and would corrupt silently. Mitigation: the refusal is loud and its message names C-28, so a provider
author who hits a false positive investigates; there is no equivalent signal for a false negative.
C-24 (fixture verification) is where a wrong emission would surface. **Open:** whether to tighten to
"refuse every query parameter" for the launch inventory, trading six blocked-but-safe operations for
the elimination of the false-negative class. Recommendation is the narrow scope, because the six
operations are the ones that make the zendesk connector partially useful today (§5).

**The flux change may land in a different shape.** flux's maintainer may prefer the `urlencode` op —
it is a smaller diff against a stable contract, and that is a legitimate call. If so: the emitter
keeps `$sep`/`when` and adds one `call urlencode` per query value; §3.3's encoding semantics
(rules 2, 5) still apply verbatim; rules 1, 3, 6 and 7 become moot; and the `0`/`false` truthiness
bug survives and needs its own story. The refusal in §4 is unchanged either way — it is a refusal to
emit *anything* unencoded, not a refusal to emit one particular shape.

**Flux floor detection is unspecified.** The emitter must know whether the target flux has `query`
before it stops refusing. There is no mechanism for that today; C-13 (`build`) has no flux-version
input. Simplest answer: the refusal is removed by a code change gated on the workspace's pinned
`codewandler-flux-lang` version, which is a deliberate, reviewed bump (AGENTS.md). **Open:** whether
`connector.toml` should record a minimum flux version so an installed module fails loudly on an old
runtime instead of silently dropping filters. Worth a story regardless of this one.

**`%` in a caller value is unhandled in either direction.** `?query=100%` parses today and reaches
the wire as an invalid percent-escape (probe, §2.3). A caller value that already contains `%20` is
indistinguishable from an encoded space. The structured `query` parameter fixes this by construction
(`%` → `%25`), but it is worth stating that the current behaviour is undefined rather than merely
ugly.

**Path parameters have the same gap, and it is not fixed here.** `path_template`
(`crates/connector-flux/src/op.rs:258+`) interpolates path values verbatim too, so a string path
parameter containing `/` or `?` escapes its segment. The launch inventory's path parameters are all
numeric ids (`ticket_id`), so nothing is broken today, and `http.request`'s structured `query` does
not cover paths. **Deliberately out of scope for C-28** — filed as an observation for whoever adds
the first string-valued path parameter. It needs the same encoder and, if the `urlencode` op is
chosen over the structured parameter, it is the one case the op form handles better.

**The zendesk plugin source is unavailable.** §Provenance. Nothing above depends on it, but
[provider-operation-inventory.md](provider-operation-inventory.md)'s zendesk citations
(`main.rs:*`) cannot currently be re-verified by anyone. That affects C-17 and C-18 more than it
affects this story; flagged, not fixed.

---

## 5. Is `zendesk.ticket.search` workable today?

**No. It must be refused.** Stated plainly for C-17, who is authoring the provider now.

### 5.1 Why not

Its `query` parameter is a required, free-form Zendesk search expression
([provider-operation-inventory.md](provider-operation-inventory.md) §3.3 — *"Zendesk search
expression"*, required). Free-form text from an agent or a support ticket routinely contains:

- `#` — `"refund for ticket #4521"` → everything after `#` becomes a fragment and **never leaves the
  process**. The vendor is asked a different question and answers it successfully.
- `&` — `"billing & invoicing"` → the value truncates at `billing ` and an empty parameter named
  ` invoicing` is invented.
- `+` — silently becomes a space.

And the injection case: a value ending `&per_page=1` overrides the operation's own page bound.

### 5.2 What C-17 should do

1. **Do not include `zendesk.ticket.search` in the emitted set.** The emitter will refuse it once
   §4's `UnencodableQueryValue` exists; until then, leaving it out of the provider TOML is
   equivalent and is the honest state.
2. **The other six operations are unaffected.** `zendesk.test` takes no parameters;
   `ticket.show`, `ticket.comment.list`, `ticket.update`, `ticket.comment.add` and `ticket.tag.add`
   take numeric path ids and numeric page bounds (§3.2, §3.3). None can carry a corrupting
   character. The zendesk connector is **6/7 today**, blocked on one operation.
3. **Record it as blocked-on-C-28 in the provider, not as missing.** A reader must be able to see
   that the seventh operation is deliberately absent and why.

### 5.3 The one thing that is *not* broken

The canonical expression from C-28's own Notes, `type:ticket status:new`, **works today** — space is
percent-encoded by `url::Url::parse` and colons are legal in a query. Do not let that mislead
anyone into reopening this: it is the narrowest possible happy path, and §2.3(a) explains why it is
the most dangerous fact in this document.

---

## 6. Second question: no optional composite-op parameter

**Confirmed, and it is a larger change than C-28's note implies.**

`composite_signature` puts every declared parameter in `required_params` and leaves
`optional_params` empty (`../flux/crates/flux-flow/src/registry.rs:183-184`, verbatim). Two
consequences, both verified:

- **Run time:** `execute_composite_call` iterates `composite.params` and returns
  `"composite op `{name}` missing required param `{param}`"` for any absent argument
  (`../flux/crates/flux-lang/src/runtime.rs:554-560`).
- **Analysis time:** `check_call_args` emits a missing-param diagnostic for each `required_params`
  key not present in the call's object literal (`../flux/crates/flux-lang/src/analyze.rs:552`,
  `:576-578`).

**The note under-states the cost.** `flux_lang::ast::Param` is
`{ name: SymbolName, ty: TypeRef }` (`../flux/crates/flux-lang/src/ast.rs:323-326`) — there is **no
optionality field anywhere in the AST**. So this is not a `registry.rs` two-liner: it is an AST
field, a parser change, a formatter change, a `composite_signature` change and an
`execute_composite_call` change, plus a serde default for AST back-compatibility (the precedent is
`jq_optional_default`, `ast.rs:328-333`).

### Judgement: a real flux gap, worth a story, but **not** on C-28's critical path

The emitter's existing workaround — declare every parameter, let the caller pass `null`, let a
`when` guard turn null into "not sent" (`crates/connector-flux/src/op.rs:47-56`) — is adequate for
**wire correctness** and should stay. Two costs remain, and they are not equal:

**(a) Value precision — real, and solved elsewhere.** The `when` guard is truthiness, so a
deliberate `0` or `false` is treated as absent and `?offset=0` is unsendable
(`crates/connector-flux/src/op.rs:54-56`). This is a genuine correctness bug — and the structured
`query` parameter (§3.2, rule 3) **fixes it for query parameters** by distinguishing null from
falsy. It therefore does not motivate the optional-param story.

**(b) Model ergonomics — real, and not solved elsewhere.** The advertised signature says all six
filters are required (`param_signature`, `../flux/crates/flux-lang/src/opspec.rs:268-280`). A model
told six parameters are required will supply six values, and the failure mode is not an error — it
is an invented empty string or a plausible-looking default reaching the vendor. That is a
correctness risk arising from a schema that lies, and no emitter-side workaround reaches it, because
the lie is in the signature flux advertises.

So: **file it, keep it separate, rank it below the encoding story.** It blocks nothing at launch —
the seven zendesk operations remain callable with explicit nulls — but it degrades every generated
op with more than one optional filter, and the degradation grows with the catalogue. The draft is
F-2 in [query-encoding-flux-stories.md](query-encoding-flux-stories.md).

---

## Acceptance / done

- [x] The gap is confirmed against flux's registered-op catalog, `ToolSpec` census and the `expr`
      builtin whitelist — with citations, not assumption (§2).
- [x] A definite answer on connector-side solvability: **no**, with each candidate refuted (§3.1).
- [x] The flux change is specified precisely, both shapes weighed, one recommended (§3.2, §3.3).
- [x] Connector-side behaviour specified: today, after, and what must be refused meanwhile (§4).
- [x] `zendesk.ticket.search` answered plainly for C-17: **blocked**, with the reason and the 6/7
      fallback (§5).
- [x] The optional-composite-parameter question confirmed and judged (§6).
- [ ] The flux stories are pasted into `../flux/docs/stories/` by a human and renumbered if needed —
      **out of this repo's control**; the drafts are ready in
      [query-encoding-flux-stories.md](query-encoding-flux-stories.md).
- [ ] The `UnencodableQueryValue` refusal is implemented in `connector-flux` — **a separate
      implementation story**, since C-28 is investigation-and-specification and this repo's crates
      are owned by other in-flight work.

---

## Appendix: the `url` probe

Executed against `url` 2.5.x (flux's pin, `../flux/Cargo.lock:6930-6932`) in a throwaway crate — not
against flux's tree, which was not modified. Re-runnable:

```rust
fn main() {
    for raw in [
        "https://acme.zendesk.com/api/v2/search.json?query=type:ticket status:new",
        "https://acme.zendesk.com/api/v2/search.json?query=billing & invoicing",
        "https://acme.zendesk.com/api/v2/search.json?query=refund for ticket #4521",
        "https://acme.zendesk.com/api/v2/search.json?query=a+b",
        "https://acme.zendesk.com/api/v2/search.json?query=a\nb",
        "https://acme.zendesk.com/api/v2/search.json?query=x&per_page=1&admin=true",
        "https://acme.zendesk.com/api/v2/search.json?query=a%20b",
    ] {
        let u = url::Url::parse(raw).unwrap();
        println!("{raw:?}\n  -> {u}\n     query={:?} fragment={:?} pairs={:?}",
                 u.query(), u.fragment(), u.query_pairs().collect::<Vec<_>>());
    }
    let mut u = url::Url::parse("https://acme.zendesk.com/api/v2/search.json").unwrap();
    u.query_pairs_mut().append_pair("query", "type:ticket status:new");
    println!("append_pair -> {u}");   // query=type%3Aticket+status%3Anew  <- form-encoded, not RFC 3986
}
```
