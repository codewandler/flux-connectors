---
id: C-115
title: "Request construction, delegation to http.request, and the mirrored network gate"
pillar: Bridge
status: ready
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

- [ ] `Tool::execute` builds `{ method, url, headers, body }` from the operation's IR and the caller's
      params, then returns `self.http.execute(ctx, request).await`, passing the **same** `ctx`.
- [ ] Path parameters, query parameters and body fields land where the IR says — the same wire-path
      nesting the emitter already honours, so `zendesk-ticket-comment-add` nests under
      `ticket.comment` rather than flattening.
- [ ] **The network gate is mirrored.** Every Tool implements `permission_subjects` returning the
      request URL and `intents` raising `NetworkFetch`, matching `HttpRequestTool`'s own
      (`crates/flux-web/src/http.rs:118` and `:126`).
- [ ] **Failing-first test:** `every_tool_declares_the_host_it_reaches` — for every shipped operation,
      assert `permission_subjects(&params)` is non-empty and names the provider's declared host. It
      must fail against an implementation that returns `Vec::new()`, which is the default the trait
      hands you for free.
- [ ] A test asserts the constructed request for at least one nested-body operation and one
      query-string operation, so a flattening or a missing `?`/`&` separator is caught here rather
      than by a vendor answering `200` and ignoring the call.
- [ ] The gate is green.

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
