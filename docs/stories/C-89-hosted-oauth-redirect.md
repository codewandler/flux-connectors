---
id: C-89
title: The hosted OAuth redirect has no home — OAuthRedirect is loopback-only
pillar: Bridge
status: ready
priority: 5
design: docs/designs/connector-configuration.md
epic: connector-config
areas: [connector-spec, bridge]
note: "OAuthRedirect is {port, path} — a CLI shape. A hosted callback is https://app.example.com/oauth/callback, supplied by the host, and often must be pre-registered in the vendor's dashboard before the flow works at all"
---

# The hosted OAuth redirect has no home — OAuthRedirect is loopback-only

## Goal
Make a hosted OAuth flow expressible. `OAuthRedirect { port, path }` describes flux's CLI login — bind
a loopback port, wait for the browser. A product's callback is a public HTTPS URL it owns, and no field
in the model can hold or constrain one.

## Acceptance
- [ ] The connector declares what it **requires of** a redirect, never the redirect itself: that one
      exists, that it must be HTTPS, and — the operationally important part — whether the vendor
      requires it **pre-registered** in their dashboard before the flow works at all.
- [ ] The URL stays the host's. The same rule the webhook callback follows in
      [C-86](C-86-connector-configuration-epic.md): a connector that carried a URL would be describing
      someone else's infrastructure.
- [ ] Pre-registration, where required, surfaces as setup instructions a product can render — the
      redirect URI is a value the *product* must give the *vendor*, which is the mirror of every other
      field in the configuration model and the one a first attempt is most likely to miss.
- [ ] The loopback case keeps working unchanged. flux's `flux auth login` binds a port and is not
      going away; this is an additional shape, not a replacement.
- [ ] Recorded whether the existing `OAuthRedirect` is extended or joined by a sibling, and why.

## Progress
- Not started. Filed 2026-07-30 while landing [C-86](C-86-connector-configuration-epic.md), on finding
  that the config model could express `oauth.client_id` and `oauth.client_secret` but nothing about
  where the grant returns to.

## Notes
- **Verified in flux as of 2026-07-30:** `crates/flux-cli/src/auth_cmd.rs` binds `127.0.0.1:{port}`,
  prints the authorize URL rather than opening a browser, serves a small completion page and verifies
  the CSRF `state`. There is no device-code flow. The one non-loopback precedent is the `claude`
  provider, which pastes a code back — because Anthropic's registered redirect is a hosted console page.
- Pairs with [C-88](C-88-prove-oauth2.md): proving OAuth on a provider without this proves only the
  loopback case.
