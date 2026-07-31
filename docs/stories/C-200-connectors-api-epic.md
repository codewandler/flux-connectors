---
id: C-200
title: "connectors-api — a multi-tenant connector host (epic)"
pillar: Bridge
status: ready
priority: 1
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, host]
note: "EPIC — owner-directed 2026-07-31: a deployed service with accounts, Google sign-in, per-tenant credentials, OAuth2 connect flows and an operation playground. Supersedes the loopback narrowing in connectors-app.md and requires the vision.md charter amendment in C-201"
---

# `connectors-api` — a multi-tenant connector host (epic)

## Goal

Stand up a running service that an operator signs into, connects providers to, and calls operations
from — so that this repository's 248 compiled operations become something a person uses rather than
something a test asserts about.

## Why now

Everything below the host already exists and is tested: `connector-pack` projects an operation onto a
flux `ToolSpec`, evaluates `{ method, url, headers, body }` from the operation's own emitted Flux,
resolves the credential through a bound `SecretStore`, registers it with flux's redactor, verifies
the registration took, places it per its declared scheme, and delegates the send to flux's
`http.request`. **No caller exists.** `ToolRegistry::new()` appears only in tests.

Two things measured on 2026-07-31 make this cheaper than the backlog implies:

- `codewandler-flux-web` **is published at 0.41.1**, matching this repository's pinned flux 0.41. The
  `Egress` seam therefore has a shipping implementation, which
  [connectors-app.md](../designs/connectors-app.md) recorded as unknown and worth finding out loudly.
- Every one of the 44 providers now declares an `authority` (C-92), so credential addressing is
  complete. The "only seven providers can authenticate" measurement in that design is stale.

## What this epic is not

It is **not** [connectors-app.md](../designs/connectors-app.md)'s loopback reference host. That design
is narrowed to one operator on `127.0.0.1` with no second principal, deliberately, because a
multi-tenant credential-holding service is the confused-deputy machine
[connectors-proxy.md](../designs/connectors-proxy.md) analysed and C-34 rejected. The owner has
directed the wider shape; [C-201](C-201-charter-multi-tenant-host.md) is where that becomes a recorded
amendment rather than a contradiction, and it must land with the epic.

## Children

| Story | Track | What it delivers |
|---|---|---|
| [C-201](C-201-charter-multi-tenant-host.md) | charter | The vision amendment and the redone confused-deputy analysis |
| [C-202](C-202-flux-web-egress.md) | A | `flux-web` in the graph; `Egress` over a real `http.request` |
| [C-203](C-203-connectors-api-skeleton.md) | A | The service, the tenancy model, one live call with a pasted token |
| [C-204](C-204-google-signin-accounts.md) | A | Google OIDC sign-in, accounts, sessions |
| [C-205](C-205-per-tenant-credential-isolation.md) | A | Per-tenant credential isolation behind `SecretStore` |
| [C-206](C-206-declare-oauth2-surface.md) | B | `[auth.oauth2]` for google, anthropic, slack |
| [C-207](C-207-oauth2-connect-flow.md) | B | PKCE, callback, token into the store |
| [C-208](C-208-operator-web-app.md) | A | Explorer, connect, and the operation playground |

Tracks A and B run in parallel and converge at C-207, where the "Connect" button replaces
paste-a-token.

## Acceptance

- [ ] A person can start the service, sign in with Google, connect at least one provider through an
      OAuth2 flow, and run an operation against the real vendor from a browser.
- [ ] Every child story is `done`, or explicitly deferred with its reason recorded here.
- [ ] The charter amendment (C-201) is merged, so no document in the repository claims this service
      may not exist.
- [ ] Two tenants' credentials are proved isolated by test, not by inspection.

## Notes

- The pack's own safety properties must not be re-implemented here. This service **binds ports and
  calls `pack`**; it constructs no request of its own. A second request path is the drift
  [connectors-app.md](../designs/connectors-app.md) refused and [C-117](C-117-pack-codegen.md) exists
  to catch.
- [C-87](C-87-configuration-codegen.md) blocks an *external* host from rendering a connect form or
  building an authorize URL, because `site.rs` collapses the whole `OAuth2Spec` to `oauth2: bool`.
  This service is in-workspace and can read the IR through `connector-spec`, so C-87 is not a blocker
  for it — but shipping without C-87 means the service is the only thing that can do this.
