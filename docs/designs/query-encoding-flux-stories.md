# Handoff: ready-to-paste flux stories for query percent-encoding

> **This file is a handoff artifact, not a tracked backlog.** Nothing in it is a story on *this*
> repo's board, and `/track:board` must never pick these up. Each block below is a complete story
> file destined for **`../flux`**'s `docs/stories/`. A human copies a block verbatim into
> `/home/timo/projects/flux/docs/stories/<id>-<slug>.md` and runs flux's own `/track:board`.
>
> Source design: [query-encoding.md](query-encoding.md) · Parent story:
> [C-28](../stories/C-28-query-percent-encoding.md)

## Why this is a sibling of `auth-seam-flux-stories.md`, not an append to it

[auth-seam-flux-stories.md](auth-seam-flux-stories.md) is scoped by its own header to the outbound
`$auth` seam: its provenance note, its ID block, its sequencing diagram and its "safety constraint,
stated once" all describe that one epic, sourced from [auth-seam.md](auth-seam.md) under C-16.
Appending an unrelated epic would make that file's framing false, and the two epics are pasted into
flux at different times by different decisions. So: a sibling, in the same style.

They are not independent, though — see the C-271 note in F-1. **A human pasting C-271 should read
F-1 first**, because C-271's Notes currently recommend an encoding function that is wrong for query
*values*.

## Before you paste

- **IDs are provisional.** The highest `C-` id in flux at the time of writing is **C-265**, and
  [auth-seam-flux-stories.md](auth-seam-flux-stories.md) already claims **C-266 … C-276**, so these
  claim **C-277 … C-278**. flux's fleet allocates ids concurrently — re-check with
  `ls ../flux/docs/stories | grep -oP '^C-\d+'` and renumber the block (and its cross-references) if
  that range is taken.
- **A design doc must exist in flux** before these land, since each sets
  `design: docs/designs/query-percent-encoding.md`. Either port
  [query-encoding.md](query-encoding.md) into flux under that name, or drop the `design:` line from
  each block. Do not leave a `design:` pointing at a file that does not exist in flux.
- **Line numbers describe flux at `v0.38.0`** (`git describe --tags`, clean worktree). Re-grep by
  symbol if a number does not land.
- **Layer facts these stories rely on** (verified at `v0.38.0`, `crates/flux-codegate/src/lib.rs`):
  `flux-system` is L2 (`:44`) and `flux-web` is L5 (`:51`), so a flux-web → flux-system dependency is
  legal and already exists. `flux-credentials` and `flux-providers` are L1 (`:41`) and therefore
  **cannot** take an L2 dependency — which is why F-1 deduplicates only two of the four existing
  percent-encoders and files the rest as a note rather than pretending it is one change.

## Sequencing

```
F-1 (C-277) structured `query` on http.request   ← blocks flux-connectors' zendesk.ticket.search
      │
      └── makes C-271 (outbound $auth Query scheme) SMALLER, not larger

F-2 (C-278) optional composite-op parameters      ← independent, lower priority, blocks nothing
```

**F-1 is on the critical path for flux-connectors' zendesk connector.** Without it,
`zendesk.ticket.search` — one of the seven operations that connector must cover to replace the
zendesk plugin — cannot be emitted correctly and is refused. See
[query-encoding.md](query-encoding.md) §5.

**F-2 blocks nothing.** It is filed because the emitter's workaround (declare everything, pass
`null`, guard with `when`) fixes the wire but not the *advertised schema*, and a schema that says
six filters are required is a schema that makes a model invent five values. See
[query-encoding.md](query-encoding.md) §6.

---

## F-1 → `C-277-http-request-structured-query-parameter.md`

```markdown
---
id: C-277
title: "`http.request` takes a structured `query` map and percent-encodes it (RFC 3986)"
pillar: Core
status: ready
priority: 3
epic: query-percent-encoding
design: docs/designs/query-percent-encoding.md
note: "flux registers no URL-encoding op at all — a query value built into `url` today can inject additional query parameters"
---

# `http.request` takes a structured `query` map and percent-encodes it

## Goal
Make a caller-supplied query value reach the wire intact, and make it impossible for that value to
add or override query parameters on the request. Today the only way to put a parameter on a URL is
to build it into the `url` string, and **nothing in flux can encode it**: the `expr` built-in
whitelist has 21 functions and no encoder (`crates/flux-lang/src/expr.rs:804-828`), and a sweep of
every registered `ToolSpec` name across `crates/` for `encode|escape|url|quote|uri` returns nothing.
`crates/flux-flow/docs/ops-reference.md:15-88` confirms the same by documentation.

The result is not merely cosmetic. `http.request` parses the assembled URL with `url::Url::parse`
(`crates/flux-system/src/net.rs:126`, reached from `crates/flux-web/src/http.rs:157`), and that parse
percent-encodes a space but **not** `&`, `#` or `+`. Measured against `url` 2.5.x:

| value interpolated into `?query=…` | what is actually sent |
|---|---|
| `type:ticket status:new` | `query=type:ticket%20status:new` — correct, by accident |
| `billing & invoicing` | `query=billing ` **plus an invented parameter** ` invoicing=` |
| `refund for ticket #4521` | `query=refund for ticket ` — the rest becomes a fragment and never leaves the process |
| `a+b` | decodes as `a b` |
| `x&per_page=1&admin=true` | three parameters: `query=x`, `per_page=1`, `admin=true` |

The last row is the reason this is filed at priority 3 rather than as a nicety: `http.request` is a
model-visible tool, so a model-supplied (or prompt-injected) value can widen a page cap, flip a
pinned boolean, or — once C-271 lands `AuthScheme::Query` — collide with the parameter carrying a
credential.

## Acceptance
- [ ] `http.request`'s input schema (`crates/flux-web/src/http.rs:90-106`) gains an optional
      `query` object. `"required"` stays `["url"]` — this is purely additive and no existing caller
      changes.
- [ ] Each key and value is percent-encoded per **RFC 3986**: `A-Za-z0-9-._~` pass through, every
      other byte becomes `%XX` with uppercase hex. **Space becomes `%20`, never `+`.**
- [ ] The parameters are appended **before** `guard_url_scoped_pinned`
      (`crates/flux-web/src/http.rs:157`), so the URL the SSRF guard vets and pins is the URL
      actually sent. A design that appends afterwards is wrong and must fail review.
- [ ] A `null` value **omits** the parameter entirely. A `false` or `0` value is **sent** — the
      distinction is null-vs-absent, not truthiness.
- [ ] Number and boolean values use their JSON scalar text, then encode.
- [ ] An array or object value is a **caller error**, not a guess. Vendor conventions for repeated
      parameters are irreconcilable (`a=1&a=2` vs `a=1,2` vs `a[]=1`) and picking one silently is the
      exact class of bug this story exists to remove.
- [ ] A `query` key that is **already present** in `url`'s query string is an error, not a silent
      duplicate.
- [ ] **Failing-first test:
      `query_map_values_are_rfc3986_percent_encoded_not_interpolated`** — issue an
      `http.request` with `url: "https://example.com/search"` and
      `query: {"q": "billing & invoicing #42"}`, and assert the URL that reached the transport is
      exactly `https://example.com/search?q=billing%20%26%20invoicing%20%2342` — one parameter, no
      fragment. Before the change the `query` argument is ignored entirely (`execute` reads only
      `url`, `method`, `headers`, `body`, `timeout` — `crates/flux-web/src/http.rs:137-160`), so the
      request goes to `https://example.com/search` with no query at all and the assertion fails.
- [ ] Second test: `query_value_cannot_inject_additional_parameters` — a value of
      `x&per_page=1&admin=true` produces **one** parameter whose decoded value is the whole string.
      This is the security-relevant assertion and must not be dropped if the story is trimmed.
- [ ] Third test: `query_encodes_space_as_percent_twenty_not_plus` — asserts `a b` becomes `a%20b`.
      Guards against a later "simplification" to `url::Url::query_pairs_mut().append_pair`, which
      form-encodes (`a+b`) and is wrong for a value a server percent-decodes.
- [ ] Fourth test: `query_null_value_is_omitted_but_false_and_zero_are_sent`.
- [ ] The encoder is **one shared function, not a fifth copy.** flux already has four private,
      effectively identical implementations: `percent_encode_component`
      (`crates/flux-plugin/src/host.rs:1843-1854`), `percent_encode_segment`
      (`crates/flux-providers/src/bedrock.rs:552-562`), `urlencode`
      (`crates/flux-credentials/src/lib.rs:1435-1446`) and `urlencode`
      (`plugins/jira/src/operations/mod.rs:857`). Promote one into `flux-system::net` (L2;
      flux-web is L5 and already depends on it — `crates/flux-codegate/src/lib.rs:44`, `:51`) and
      have `flux-plugin` call it, deleting its private copy.
- [ ] The op's schema `description` (`crates/flux-web/src/http.rs:88`, `:99`) documents `query` and
      says plainly that a value built into `url` is **not** encoded.
- [ ] `crates/flux-flow/docs/ops-reference.md`'s `http.request` row is updated with the new
      argument.
- [ ] `cargo run -p flux-codegate` (or the repo's layering check) stays green.

## Progress
- (not started)

## Notes
- **Read this before implementing C-271.** C-271's Notes
  (flux-connectors' `docs/designs/auth-seam-flux-stories.md`, F-6) recommend
  `url::Url::query_pairs_mut().append_pair(..)` for appending the `Query`-scheme credential. That
  function serializes as `application/x-www-form-urlencoded`: `append_pair("query", "type:ticket
  status:new")` yields `query=type%3Aticket+status%3Anew`. For a *credential* value that is usually
  harmless; for a caller's query value it is wrong, because a server that percent-decodes per
  RFC 3986 reads the `+` as a literal plus. RFC 3986 encoding is correct under **both** decoders
  (`%20` form-decodes to a space too), so this story's encoder is the safe one for both paths.
- **This story makes C-271 smaller.** C-271's Acceptance adds a request-level `auth` array
  specifically because *"`http.request` has no `query` parameter"*. Landing this first gives both
  stories one append path.
- **Ordering with C-271, once both exist:** auth parameters apply **after** caller `query`
  parameters, and a caller `query` key colliding with an auth parameter name must be refused
  **before the credential is resolved**. Without that rule a model-supplied
  `query: {"api_key": "…"}` interacts with credential injection.
- **Not in scope:** path-segment encoding. A string interpolated into the *path* has the same
  problem and this story does not address it. Worth a follow-up when a caller needs it; flux's own
  `percent_encode_segment` (bedrock, above) is the reference behaviour.
- **Deliberately not deduplicated here:** `flux-credentials` and `flux-providers` are L1
  (`crates/flux-codegate/src/lib.rs:41`) and cannot depend on an L2 `flux-system`. Folding those two
  copies in needs an L0 home and is a separate cleanup — file it if you want it, but do not let it
  grow this story.
- **The alternative that was rejected, and why**, so it is not re-proposed: a pure `urlencode` op
  registered alongside `html_to_markdown` (`crates/flux-web/src/fetch.rs:261-286`) is a smaller diff
  against a stable contract, and it does fix correctness. It was rejected because it is *opt-in*:
  the caller still builds the URL by string concatenation, so the injection vector above stays open
  for every caller who forgets — including a model calling `http.request` directly. If flux prefers
  that shape anyway, say so on this story; flux-connectors will follow it, and the encoding rules
  above (RFC 3986, arrays refused) still apply verbatim.
```

---

## F-2 → `C-278-optional-composite-op-parameters.md`

```markdown
---
id: C-278
title: Composite ops need optional parameters — today every declared param is required
pillar: Core
status: ready
priority: 5
epic: query-percent-encoding
design: docs/designs/query-percent-encoding.md
note: "the advertised signature says every filter is required, so a model filtering on one field is told to supply six"
---

# Composite ops need optional parameters

## Goal
Let a composite op declare a parameter a caller may omit, so a six-filter operation can be called
with one argument. Today every declared parameter is required in both directions:

- **Signature:** `composite_signature` puts every param in `required_params` and leaves
  `optional_params` empty (`crates/flux-flow/src/registry.rs:183-184`).
- **Run time:** `execute_composite_call` iterates `composite.params` and returns
  ``composite op `{name}` missing required param `{param}` `` for any absent argument
  (`crates/flux-lang/src/runtime.rs:554-560`).
- **Analysis time:** `check_call_args` emits a missing-param diagnostic for each `required_params`
  key absent from the call's object literal (`crates/flux-lang/src/analyze.rs:552`, `:576-578`).

The cost is not verbosity — it is that the **advertised schema lies**. `param_signature`
(`crates/flux-lang/src/opspec.rs:268-280`) renders every parameter as required, so a model asked to
filter on one field is told it must supply six. The failure mode is not an error: the model invents
five plausible-looking values and the vendor receives them.

## Why it is not a one-line change
`flux_lang::ast::Param` is `{ name: SymbolName, ty: TypeRef }`
(`crates/flux-lang/src/ast.rs:323-326`) — there is **no optionality field anywhere in the AST**. So
this touches the AST, the parser, the formatter, `composite_signature` and
`execute_composite_call`, plus a serde default for AST back-compatibility. `Param` derives
`Serialize`/`Deserialize`/`JsonSchema`, so an older serialized AST must keep deserializing; the
established precedent for exactly this is `jq_optional_default` (`crates/flux-lang/src/ast.rs:328-333`).

## Acceptance
- [ ] `flux_lang::ast::Param` carries optionality, defaulting (via serde) to **required** so every
      existing serialized AST deserializes unchanged and behaves identically.
- [ ] The surface syntax round-trips: parser accepts it, `format` re-emits it, and
      parse → format → parse is a fixed point. (Spelling is this story's choice — `?` after the name,
      as in `op search(query: String, page?: Number)`, matches the type-annotation grammar's shape
      and reads at a glance.)
- [ ] `composite_signature` (`crates/flux-flow/src/registry.rs:180-185`) routes optional params into
      `optional_params` instead of `required_params`, so `param_signature`
      (`crates/flux-lang/src/opspec.rs:268-280`) and every schema derived from it stop advertising
      them as required.
- [ ] `execute_composite_call` (`crates/flux-lang/src/runtime.rs:553-575`) does not error on an
      absent optional argument. An omitted optional parameter is **unbound**, so a `when $page`
      guard treats it as absent — the same behaviour a caller gets today by passing `null`.
- [ ] A missing **required** param still errors with the same message, unchanged.
- [ ] **Failing-first test: `composite_op_with_an_optional_param_runs_when_that_arg_is_omitted`** —
      declare `op search(query: String, page?: Number)` whose body binds `$out = fmt("{query}")`,
      call it with `{query: "x"}` only, and assert it succeeds. Today it fails with
      ``composite op `search` missing required param `page` `` — and, before the AST field exists,
      the declaration does not even parse.
- [ ] Second test: `optional_composite_param_is_advertised_as_optional_not_required` — the
      `OpSignature` for that op has `page` in `optional_params` and not in `required_params`. This is
      the assertion that actually fixes model behaviour; the run-time one alone does not.
- [ ] Third test: `omitting_a_required_param_still_errors` — the existing behaviour is unchanged for
      required params.
- [ ] Fourth test: `an_optional_param_round_trips_through_parse_format_parse`.
- [ ] `crates/flux-lang/docs/reference.md` documents the syntax.

## Progress
- (not started)

## Notes
- **Blocks nothing; it degrades everything.** flux-connectors' generated ops work today by declaring
  every parameter and having the caller pass `null`, with a `when` guard turning null into "not
  sent". That is adequate for wire correctness and will stay as the fallback. What it cannot reach is
  the advertised signature, which is where the model actually decides what to send.
- **Do not fold in the truthiness question.** flux-connectors' `when $page` guard also treats a
  deliberate `0` or `false` as absent. That is *their* emitter's problem and C-277's structured
  `query` map solves it for query parameters (null-omitted, `0`/`false` sent). Nothing here should
  try to give `when` different semantics.
- An optional param that is *omitted* and one that is *passed as null* should behave identically —
  both unbound, both falsy to `when`. A story that makes them differ has invented a distinction
  callers cannot see.
```
