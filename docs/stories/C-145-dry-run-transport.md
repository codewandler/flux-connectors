---
id: C-145
title: "A dry-run transport that cannot send"
pillar: Bridge
status: in-progress
priority: 3
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [bridge]
note: "dry-run and 'intercept the calls for a demo' are ONE mechanism with two payloads — C-115 already takes its transport as a constructor argument, so neither forks the request path. Structurally unable to send, not a live client with a flag"
---

# A dry-run transport that cannot send

## Goal

Answer "what request would this operation make?" without making it — and give the demo and
interception paths the seam they need.

## Acceptance

- [x] A `Transport` seam in `connector-pack`, with the live implementation delegating to flux's
      `http.request` exactly as [C-115](C-115-request-delegation.md) landed it.
- [x] A **dry-run** implementation returning the constructed request — method, URL, headers, body —
      instead of sending it.
- [x] **It is structurally incapable of sending.** The dry-run type holds no HTTP client, no
      transport handle, nothing that could reach a socket. **Failing-first test:**
      `a_dry_run_transport_cannot_be_constructed_with_a_live_client` — a flag on a live client is
      something a caller forgets, and this must not be that.
- [x] A dry-run result never contains a credential value. The request it reports carries credential
      *references* as they are declared, not resolved secrets.
- [x] **The differential test [C-117](C-117-pack-codegen.md) needs lives here:** for every shipped
      operation, the dry-run request and the emitted `.flux` module's request agree on method, URL and
      body shape. A divergence fails, naming the operation.
- [x] The gate is green; the build stays a fixed point.

## Notes

- **Depends on [C-115](C-115-request-delegation.md).** It already takes its transport as a
  constructor argument — deliberately, and the review of that story recorded the trade — so this is
  an implementation of an existing seam rather than a change to it.
- **This is the same mechanism as the demo path** ([C-146](C-146-demo-fixtures.md)): a transport that
  substitutes for egress. Build one seam, not two.
- The value is not only the demo. A dry-run is checkable **offline** for all 97 operations, which is
  exactly what the pack-vs-module differential wants and what a reviewer can run without a vendor
  account.
- Keep the refusal discipline: a request the pack cannot construct must fail loudly rather than being
  reported as an empty or partial dry-run. A partly-built request is a *different* call, and reporting
  it as the real one would be worse than reporting nothing.

## Progress

**Landed.** `crates/connector-pack/src/dry_run.rs` carries the seam and the transport;
`tests/dry_run.rs` and `tests/differential.rs` carry the proofs.

**The one design decision worth re-reading.** A dry run is *not* an `Egress` holding a stand-in
tool, which is the obvious implementation. `Egress` is reached at the **end** of the request path,
after `build_authenticated_request` has pulled a tenant's credential out of the store and placed it
— so a stand-in there reports resolved plaintext, and C-159's redacting `Debug` makes that *look*
handled. The dry run therefore sits upstream of resolution: `build_request` (no credential), then
each declared credential's **reference** placed through the real `auth::place`. The store is never
consulted and no `ToolContext` is on the path, so absence is a property of the code path rather than
of a scrub applied afterwards.

**The reference is `~credential.<name>~`**, spelled entirely from RFC 3986's unreserved set so that
`auth::placed_form`'s percent-encoder is the identity over it. That is what lets the dry run reuse
the real placement code — same header names, same prefixes, same `?`/`&` separator — instead of a
second copy that would agree until it did not.

**The differential found nothing.** All 254 shipped operations agree between
`crates/catalog/ops/<provider>/<id>.flux` (what the pack evaluates) and `connectors/*.flux` (what
the repository ships as the human-readable contract) on method, URL, headers and body. Those two
artifacts had never been compared before; `catalog::Operation::flux`'s claim that they are the same
bytes is now checked rather than trusted. `a_divergence_is_reported_and_names_the_operation` is the
control that keeps the green run from meaning "this test cannot tell".

### What adopting this in `crates/connectors-api` would take

Deliberately not done here — that crate was owned by another agent this wave. The seam was designed
so the adoption is small:

- `exec.rs` currently goes through the registry (`pack(...)` then `tool.execute(&ctx, params)`),
  which is the **live** arm by construction. A dry run does not need the registry at all:
  `Operation::project(entry, app.egress(), credentials, configuration)?.dry_run(&params)?` returns a
  `DryRun` with no `ctx`, no store read and no vendor call. That is roughly a twenty-line
  `exec::dry_run` beside `exec::execute`, plus a `POST /operations/:id/dry-run` route and a button
  in `ui.rs`.
- `DryRun::to_json()` is already the response body: `{ operation, tool, request: {url, method,
  headers?, body?}, credentials: [{credential, reference, place, target, prefix}] }`.
- **One wart worth closing in that story rather than here.** `Operation::project` still requires an
  `Egress` and a `Credentials`, neither of which a dry run uses, so a host wanting only rehearsals
  must construct a live client it never calls. Closing it means splitting the transport-free half of
  `Operation` (entry, spec, declaration, settings snapshot) into a private projection and exposing a
  `DryRunTransport::project(entry, configuration)` over it. That is additive and mechanical; it was
  left out because it is not in this story's Acceptance and `Operation::egress()` is published API.
