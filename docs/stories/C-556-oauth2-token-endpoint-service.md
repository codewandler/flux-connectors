---
id: C-556
title: "An OAuth2 declaration may place its token endpoint on a second service"
pillar: Codegen
status: ready
priority: 1
epic: catalog-artifact
areas: [connector-spec, connector-cli]
note: "C-555's measured gap: OAuth2Spec binds authorize and token to ONE declared service, so Anthropic's subscription flow (authorize on claude.ai, token on platform.claude.com) is inexpressible. The fix is an optional second service REFERENCE — a name, never a URL, so http_hosts and the declared-authority rules keep working"
---

# An OAuth2 declaration may place its token endpoint on a second service

## Goal

`OAuth2Spec` gains an optional `token_endpoint` — the declared name of a second service whose base
URL the `token_path` resolves against, defaulting to the existing single `endpoint` when absent.
A name, never a URL: the host set stays derived from declared services, so `http_hosts`,
declared-authority validation and X-154's `NoDeclaredDefault` composition rule all keep working
unchanged. This is the loader/spec extension C-555 stopped at, specified from its finding.

## Acceptance

- [ ] `OAuth2Spec` carries `token_endpoint: Option<String>` (or the loader's idiomatic
      equivalent), validated like `endpoint`: it must name a declared service of the same
      connector, and a dangling name is a loud loader refusal. Absent means today's behaviour,
      byte-for-byte — every existing declaration is unaffected, proven by the committed documents
      not moving.
- [ ] The canonical document, the manifest serialization, and `catalog::OAuth2` all carry the new
      field (additive; document schema minor bump per the C-537 forward-compat contract — an older
      reader tolerates it, per the additive-growth test).
- [ ] A failing-first loader test: a two-host declaration (authorize service ≠ token service)
      loads and lowers correctly; a dangling `token_endpoint` name refuses naming it.
- [ ] The consumer contract is stated where X-154's composition reads: the token redemption
      resolves `token_path` against the token endpoint's service base URL (declared defaults
      rule applies to it identically); recorded in the field's doc and the design doc.
- [ ] Full gate green; `diff` clean (no committed artifact moves — nothing declares the field
      yet; C-555 round 2 is the first declarer).

## Progress

- 2026-08-12: Filed from C-555's measured model gap, after the operator decided to ship both
  Anthropic OAuth2 flows. The subscription flow (claude.ai authorize + platform.claude.com token)
  is the first two-host consumer; the console flow is single-host and needs nothing from here.

## Notes

- Write set: `crates/connector-spec/src/auth.rs`, the document lowering in
  `crates/connector-cli` (document.rs/catalog.rs/