---
id: C-531
title: "A hosted deployment can be asked for its redirect URI"
pillar: Spec
status: done
priority: 0
design: docs/designs/connector-configuration.md
epic: connector-config
areas: [connector-spec, providers, tests]
note: "OAuth2Spec::redirect models a loopback port and path — the native-app shape — so a service reached at https://exchange.internal/... had nowhere to put its callback"
---

# A hosted deployment can be asked for its redirect URI

## Goal

Let an operator supply the callback address their deployment serves, as part of the same OAuth
application registration that already collects a client id and a client secret.

## The gap

`OAuth2Spec::redirect` is `{ port: u16, path: String }` — a **loopback** redirect, the RFC 8252 §7.3
shape for a native application running on the operator's own machine. A hosted host reached at
`https://exchange.internal/api/oauth/callback` cannot be spelled that way at all, so a deployment
had no declared place to put its callback and would have discovered the mismatch on the vendor's
error page after a failed grant.

## Acceptance

- [x] `Binding::OAuthRedirectUri` — `oauth.redirect_uri` — deriving **operator** level and
      **not** secret. It is the third half of one registration: a client id, a client secret and a
      redirect URI are issued together, so they are supplied together, by the same person, at the
      same level.
- [x] The same refusal the client id already carried: a registration field with no `[auth.oauth2]`
      block to belong to is refused, because nothing would ever read it.
- [x] `providers/gitlab.toml` declares it, with the exact-match requirement in its `help`.
- [x] The corpus gate in `auth_archetypes.rs` requires it of **every** connector declaring a grant,
      beside the client id and secret it already required. A grant whose redirect URI cannot be
      supplied is one only a loopback deployment can complete.
- [x] The published `provider-toml.schema.json` and the loader's binding vocabulary list it.
- [x] Failing-first tests for the level/secrecy derivation and for the no-grant refusal.

## Progress

- 2026-08-11: Implemented, on the owner's instruction to fix rather than document the limitation.

## Notes

**The connector still declares no destination**, and that is the same line C-529 drew. This is a
*slot* a host fills, exactly like `endpoint.origin`: the vendor does not choose your callback, you
register it with them. What the connector supplies is the question, so an operator is asked during
setup instead of debugging a redirect mismatch afterwards.

`OAuth2Spec::redirect` is left as it is and is not deprecated. A loopback redirect is a real and
different thing — a developer running a host on their own machine — and it is a *vendor* fact
(whether the vendor permits `http://127.0.0.1`), where the registered URI is a *deployment* fact.
Both can be true at once for one connector.
