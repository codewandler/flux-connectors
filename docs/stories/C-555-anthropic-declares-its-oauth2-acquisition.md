---
id: C-555
title: "Anthropic declares its OAuth2 acquisition"
pillar: Connector
status: ready
priority: 1
epic: catalog-artifact
areas: [providers, connector-spec]
note: "The hard leg of the cross-repo login goal: Anthropic's authorize host (claude.ai) differs from its token host (console.anthropic.com), which one endpoint service base cannot express — the token-endpoint quirk surface or a deliberate model extension is the likely path. PKCE S256 is mandatory on this flow, matching what Exchange already enforces"
---

# Anthropic declares its OAuth2 acquisition

## Goal

`providers/anthropic.toml` declares the OAuth2 authorization-code acquisition (the Claude
sign-in flow: authorize on `https://claude.ai/oauth/authorize`, token on
`https://console.anthropic.com/v1/oauth/token`, PKCE S256 required), so a host can compose
Anthropic's authorize URL from the artifact and redeem the code at the declared token endpoint.
Exchange's composition (X-154) and autodev's Anthropic login are the consumers.

## Acceptance

- [ ] The two-host shape is expressed honestly within the model — an auth-host service for the
      authorize leg plus the token endpoint carried where the model puts a cross-host token URL
      (the `[[auth.quirks.token_endpoint]]` surface babelforce established is the first candidate;
      read its loader semantics before inventing anything). If the model genuinely cannot express
      it, STOP at that finding: report precisely what is missing, and the model extension becomes
      its own loader/spec story rather than an ad-hoc widening inside this one.
- [ ] Grants `authorization_code` + `refresh_token` (this flow issues refresh tokens); PKCE's
      mandatory-S256 property is recorded in the declaration's comments with its source; scopes
      are the minimal set the connector's operations need (`user:inference` for the models
      surface; each scope's reason a comment).
- [ ] No registration value in the artifact; the client id is deployment configuration (the flow
      has a well-known public client id — that FACT may be recorded in a comment, the VALUE must
      not be a declaration field the loader would refuse anyway).
- [ ] The credential relationship is deliberate: how the OAuth2-acquired token relates to the
      existing API-key credential slot(s) is decided and recorded (a sibling credential the
      operations can authenticate with, or a distinct acquisition on the existing slot) — not
      left implicit.
- [ ] Scoped provider gate green with only the documented expected staleness reds;
      `catalog/anthropic.catalog.json`'s auth block quoted in the report.
- [ ] Composability sanity: every base URL the composition needs is non-templated or carries a
      declared default (X-154's `NoDeclaredDefault` rule is the consumer contract).

## Progress

- 2026-08-12: Filed by the cross-repo coordinator for the exchange/autodev OAuth login goal.

## Notes

- Write set: `providers/anthropic.toml` + per-provider artifacts; possibly a finding against
  `connector-spec`'s loader (reported, not implemented here).
- Verify the endpoint facts against current public documentation of the flow before declaring
  them; the story's URLs are the coordinator's knowledge-cutoff recollection and must not be
  trusted un-reverified (this repository's "Before you assert anything" rule).
