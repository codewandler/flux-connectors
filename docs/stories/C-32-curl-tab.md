---
id: C-32
title: Emit a curl form for each operation
pillar: Codegen
status: ready
priority: 9
design: docs/designs/provider-docs.md
epic: provider-docs
areas: [connector-flux]
---

# Emit a curl form for each operation

## Goal
Show each operation as a raw HTTP request, so someone can sanity-check an integration without running
flux — and so the page documents the connector as well as the operation.

## Acceptance
- [ ] Each operation renders a `curl` invocation: method, URL with path parameters substituted, query
      parameters, headers, and a JSON body where one applies.
- [ ] The credential is an **env-var placeholder naming what the manifest declares**
      (`-H "Authorization: Bearer $BABELFORCE_TOKEN"`), never a value, and never a pre-composed
      header the operator would have to assemble by hand.
- [ ] Zendesk's Basic form is rendered honestly: the user half is `<email>/token`, an env value plus
      a literal suffix — not a suggestion to bake the suffix into the env var.
- [ ] The curl and the Flux fence for the same operation describe the **same request**. A test
      asserts method, URL and body agree; a page whose two tabs disagree is worse than a page with
      one.
- [ ] Query values are shown correctly encoded, or the operation is marked as blocked — see the
      encoding note below.

## Progress
- (not started)

## Notes
- **Freshdesk currently has no credential at all** (C-17): its Basic form puts the secret in the
  username position, which the IR cannot yet express safely. Render what is true — an unauthenticated
  request — rather than inventing a plausible header.
- **Percent-encoding applies here too.** C-28 established that generated query values are injectable
  and that `url::Url::parse` already rescues *spaces*, so a curl containing a space looks fine while
  `&`, `#` and `+` corrupt the request. Do not paper over it in the rendered curl.
- A second curl variant targeting the [connectors proxy](../designs/connectors-proxy.md) — with no
  secret at all — is deliberately **not** in scope. That epic is gated on a charter decision.
