---
id: C-35
title: Specify the proxy request contract and its guardrails
pillar: Bridge
status: blocked
design: docs/designs/connectors-proxy.md
epic: connectors-proxy
areas: [flux-bridge]
note: blocked on C-34
---

# Specify the proxy request contract and its guardrails

## Goal
Define how a client addresses an operation through the proxy, and the controls that keep a
credential-injecting service from becoming a credential-lending one.

## Acceptance
- [ ] The request contract is specified: a client names **provider + operation** and supplies
      parameters, never a vendor URL.
- [ ] **Host allowlist enforced from the manifest's `http_hosts`.** A request that would reach a host
      the manifest does not declare is refused. This is the control the whole design rests on —
      without it the proxy lends its credentials against any host a caller names.
- [ ] Credential scoping: an operation receives the credentials its requirement set names and no
      others.
- [ ] **The proxy is authenticated**, and refuses a non-loopback bind without a token.
- [ ] No credential appears in a log, a trace, or an error body returned to the client.
- [ ] Provider-agnostic: adding a provider means adding a manifest, never proxy code. A test proves
      it with a provider the proxy has never seen.

## Progress
- **Blocked on C-34** — the charter decision. No code before it resolves.

## Notes
- The manifest already declares everything the proxy needs: reachable hosts, credential names,
  schemes, and where each goes on the request. That is what makes agnosticism achievable rather than
  aspirational.
