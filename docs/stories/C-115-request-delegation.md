---
id: C-115
title: "Request construction, delegation to http.request, and the mirrored network gate"
pillar: Bridge
status: in-progress
priority: 2
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [bridge]
note: "SAFETY — delegating to HttpRequestTool::execute bypasses Executor::dispatch, so a Tool that fails to declare its own permission_subjects is an un-gated hole through the host's network policy"
---

# Request construction, delegation to http.request, and the mirrored network gate

## Goal

Make a projected Tool actually call the vendor: build the request from typed parameters and hand it
to flux's own `HttpRequestTool`, so **flux keeps every byte of egress** and this repository still
opens no socket.

## Acceptance

- [x] `Tool::execute` builds `{ method, url, headers, body }` from the operation's IR and the caller's
      params, then returns `self.http.execute(ctx, request).await`, passing the **same** `ctx`.
- [x] Path parameters, query parameters and body fields land where the IR says — the same wire-path
      nesting the emitter already honours, so `zendesk-ticket-comment-add` nests under
      `ticket.comment` rather than flattening.
- [x] **The network gate is mirrored.** Every Tool implements `permission_subjects` returning the
      request URL and `intents` raising `NetworkFetch`, matching `HttpRequestTool`'s own
      (`crates/flux-web/src/http.rs:118` and `:126`).
- [x] **Failing-first test:** `every_tool_declares_the_host_it_reaches` — for every shipped operation,
      assert `permission_subjects(&params)` is non-empty and names the provider's declared host. It
      must fail against an implementation that returns `Vec::new()`, which is the default the trait
      hands you for free.
- [x] A test asserts the constructed request for at least one nested-body operation and one
      query-string operation, so a flattening or a missing `?`/`&` separator is caught here rather
      than by a vendor answering `200` and ignoring the call.
- [x] The gate is green.

## Notes

- **This is the story that can be implemented wrongly while appearing to work.** `permission_subjects`
  and `intents` both have default implementations — empty and empty. A Tool that omits them compiles,
  registers, executes, and reaches the vendor, with the host's network policy never consulted for the
  inner call. The named test above is the whole defence; write it first and watch it fail.
- `flux_web::http::HttpRequestTool` is public at `crates/flux-web/src/http.rs:38`. Take it as a
  constructor argument rather than building one, so a host can supply a pre-configured instance.
- The vendor host is declared data: the manifest's `http_hosts` (C-10). Derive the subject from it
  rather than re-parsing the URL template.
- Credentials are **not** this story. Build the request with no auth applied; C-116 adds the
  credential port on top. If that makes an end-to-end call impossible here, assert on the constructed
  request rather than reaching for a real call.
- `http.request` returns one flat string (`HTTP {status}\n{headers}\n{body}`) — the constraint
  `crates/connector-flux/src/op.rs` already records. Do not attempt to field-select the response;
  return it as the `ToolResult` content and leave shaping to a later story.

## Progress

**Done.** `crates/connector-pack/` only; the gate is green and `build` reports
`18 providers, 248 artifacts up to date; nothing written`.

### The request is evaluated from the emitted Flux, not re-lowered from the IR

`src/request.rs` builds `{ method, url, headers, body }` by **evaluating the operation's own `op`
declaration** — the same statements `connectors/<provider>.flux` ships. This is the choice
`src/spec.rs` already made for the *contract*, applied to the *request*, and it is the more
load-bearing half: a contract that drifts tells a model something slightly wrong, while a request
that drifts sends a vendor a call the module would never have made. The pack's request is the
module's request by construction, so C-117's differential test has nothing left to catch here.

The evaluator is closed on purpose. `connector-flux` emits one shape, so the node set is `Bind` of
`Lit`/`Fmt`/`Obj`/`Parse`/`Call`, `When` over a `Var`, and `Return`; **anything else is
`Error::Unbuildable`, never a skip.** An emitter that grows a node this does not model — a quirk
compiled into control flow (C-12), a `retry` — must fail loudly, because a partly-evaluated request
is not a degraded request, it is a different call, and the vendor answers it. Interpolation,
truthiness and value-to-text reproduce flux-lang's `interpolate_str`, `json_truthy` and `lit_text`,
so an unbound `{name}` stays verbatim and `null` renders empty — which is what keeps a guarded
filter genuinely unsent rather than sent as `?page=null`.

### The gate cannot answer empty

`permission_subjects` returns the built URL, and falls back to the entry's declared `hosts` (C-10's
`http_hosts`, read as data rather than re-parsed from a URL template) when the request will not
build. It returns a `Vec` and cannot fail, so without the fallback the call most likely to be
malformed would be the one call nobody gates. `Error::NoDeclaredHost` then refuses at **install**
any entry that would have no fallback, which makes an empty answer unreachable rather than merely
unlikely.

C-114's tripwire was **inverted, not deleted**:
`the_network_gate_is_unmirrored_only_because_execute_is_inert` is now
`the_network_gate_is_mirrored_because_execute_reaches_the_network`, asserting the exact URL and the
exact `Intent`.

### Deviation: the transport is `Egress(Arc<dyn Tool>)`, not `HttpRequestTool`

The Acceptance names `flux_web::http::HttpRequestTool`. That type lives in `codewandler-flux-web`,
which **is not a dependency of this workspace** — naming it means adding one, and the manifests were
fenced this wave. So the transport is taken as flux's own `Tool` trait behind a newtype:
`Egress::new(Arc::new(HttpRequestTool::new(&opts)))` at the host, `self.http.tool().execute(ctx, …)`
here, same `ctx`.

Deliberate, not a shortfall, and it buys two things the concrete type could not:

- **The no-socket claim stays structural.** Naming `HttpRequestTool` would link an HTTP client, a DNS
  resolver and an SSRF guard into a library whose whole point is that it opens none.
- **A non-vendor transport plugs into the same seam** — a dry-run that renders the request instead of
  sending it, or a recorded fixture — without forking the request path.

**The named consequence:** `dyn Tool` cannot enforce that what it holds *is* `http.request`, so a
wrongly-wired host would send every connector's traffic elsewhere. Nothing in the type system closes
that, because the same openness is what a dry-run transport needs. The newtype narrows it as far as
it goes: the choice must be *stated* at the call site rather than coerced silently out of whatever
`Arc<dyn Tool>` was nearest. `Egress`' documentation carries the invariant a substitute must honour.

### Also

- Each operation's Flux is parsed **once**: `spec::project` split into `project` +
  `project_declaration`, so the spec and the request are two views of one parse that cannot disagree.
- A call missing a declared parameter is `Error::MissingParameter`, as flux's own composite dispatch
  does it. An optional parameter is one a caller may pass `null` for, not one they may omit — an
  absent path parameter would otherwise leave `{ticket_id}` verbatim in a URL the vendor answers.
- `not_wired_yet` is gone; it existed only to name this story.
- `execute` itself is not unit-tested: a real `ToolContext` needs a `flux_system::System` over a
  workspace root, and `flux-system` is not a dependency either. Its whole body is `build_request`
  plus the delegation, and `build_request` is asserted exhaustively.

### Left for the next story

Credentials (C-116) — every request is built with no auth applied. No config resolution exists yet
either, so a URL and its subject still carry `{subdomain}`/`{domain}` verbatim; an egress allow-list
written against a concrete tenant host will not match the subject this pack declares. And
`zendesk-ticket-search`'s missing percent-encoding is reproduced faithfully, bug included — it is a
recorded intentional gap, and C-144 now files the body-encoding half of the same problem.
