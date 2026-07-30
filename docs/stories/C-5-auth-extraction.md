---
id: C-5
title: Extract auth methods from securitySchemes
pillar: Spec
status: backlog
priority:
design: docs/designs/auth-seam.md
epic: connectors-v1
areas: [connector-spec]
---

# Extract auth methods from securitySchemes

## Goal
Derive a connector's auth methods from the spec's `securitySchemes`, overridable from the provider
TOML, using the same scheme vocabulary flux already has for plugins.

## Acceptance
- [ ] `http`/`bearer`, `http`/`basic`, `apiKey` in header, and `apiKey` in query map onto the IR's
      `bearer` / `basic` / `header{name}` / `query{name}` schemes.
- [ ] `oauth2` schemes are captured in the IR (flows, scopes, token URL) even though only static
      schemes are generated in v1 — the manifest schema should not need reshaping later.
- [ ] A provider TOML can override or fully replace an extracted auth method, for the common case
      where a vendor's spec misdeclares its own auth.
- [ ] Each auth method carries a `purpose` name and the env var names for its secret and, for
      `basic`, its user half — never a value.
- [ ] A test asserts no credential value can be expressed anywhere in a provider TOML.

## Progress
- (not started)

## Notes
- Zendesk is the motivating case: `basic` with `<email>/token` as the user half and the API token as
  the secret, mirroring `../flux/plugins/zendesk/src/main.rs`.
- The scheme names must match `flux_plugin_protocol::AuthScheme`
  (`../flux/crates/flux-plugin-protocol/src/lib.rs:344`) — see [auth-seam](../designs/auth-seam.md).
