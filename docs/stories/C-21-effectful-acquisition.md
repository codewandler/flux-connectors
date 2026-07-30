---
id: C-21
title: Declare effectful acquisition for host execution
pillar: Bridge
status: backlog
design: docs/designs/unified-auth.md
epic: unified-auth
areas: [connector-spec, flux-bridge]
note: OAuth2 and session login · the token never enters generated Flux
---

# Declare effectful acquisition for host execution

## Goal
Let a connector declare OAuth2 and session-login acquisition in its manifest, for the **host** to
execute — so no connector re-implements token refresh and no raw token ever enters a Flux symbol.

## Acceptance
- [ ] The manifest expresses `oauth2 { grant, token_url, refresh_url, scopes, client_ref }` for the
      grants flux already supports for plugins (`authorization_code`, `password`,
      `client_credentials`, `refresh_token`).
- [ ] `session { login_op, extract }` is expressible: call a login endpoint, extract a token from the
      response by a declared path.
- [ ] A test asserts an effectful acquisition **never** appears in generated `.flux` output — it is
      manifest-only. This is the invariant the whole story exists to protect.
- [ ] Babelforce's SSO-issued Bearer is expressible end to end, including where the token comes from.
- [ ] The design records token-cache semantics — lifetime, scope, refresh-on-401, and what happens
      when two requests refresh concurrently — even where implementation is deferred.

## Progress
- (not started)

## Notes
- flux already runs OAuth2 grants for **plugins** (`OAuth2Spec`, `OAuthGrant` in
  `../flux/crates/flux-plugin-protocol/src/lib.rs`, plus `flux auth login`). Reuse that machinery
  rather than building a second one — the point of matching flux's vocabulary is to inherit this.
- Token cache semantics are the genuinely hard part and are called out as an open question in the
  [design](../designs/unified-auth.md). Do not hand-wave them; write them down even if the code lands
  later.
- Depends on C-19.
