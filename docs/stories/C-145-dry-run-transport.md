---
id: C-145
title: "A dry-run transport that cannot send"
pillar: Bridge
status: ready
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

- [ ] A `Transport` seam in `connector-pack`, with the live implementation delegating to flux's
      `http.request` exactly as [C-115](C-115-request-delegation.md) landed it.
- [ ] A **dry-run** implementation returning the constructed request — method, URL, headers, body —
      instead of sending it.
- [ ] **It is structurally incapable of sending.** The dry-run type holds no HTTP client, no
      transport handle, nothing that could reach a socket. **Failing-first test:**
      `a_dry_run_transport_cannot_be_constructed_with_a_live_client` — a flag on a live client is
      something a caller forgets, and this must not be that.
- [ ] A dry-run result never contains a credential value. The request it reports carries credential
      *references* as they are declared, not resolved secrets.
- [ ] **The differential test [C-117](C-117-pack-codegen.md) needs lives here:** for every shipped
      operation, the dry-run request and the emitted `.flux` module's request agree on method, URL and
      body shape. A divergence fails, naming the operation.
- [ ] The gate is green; the build stays a fixed point.

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
