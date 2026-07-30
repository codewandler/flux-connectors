---
id: C-68
title: Bind a service's endpoint to operator configuration
pillar: Spec
status: ready
priority: 6
design:
epic: connectors-v1
areas: [connector-spec, connector-cli]
note: closes the SCHEMA GAP every shipped provider records in a comment
---

# Bind a service's endpoint to operator configuration

## Goal
Make a connector's endpoint a declared, bindable thing instead of a plain string with a comment:
which environment variable supplies it, which template variables it binds, and whether a sandbox
alternative exists.

## Acceptance
- [ ] A service declares its endpoint as a spec, not a string: base URL or URLs, each template
      variable bound to a named source (`ZENDESK_URL` binds `{subdomain}`), required versus optional,
      and validation. Shape it against flux's plugin `EndpointSpec`, which
      `docs/designs/auth-seam.md` already cites as the precedent.
- [ ] **The recorded gap closes.** All three original provider TOMLs carry a `SCHEMA GAP:` comment
      saying nothing declares that their `*_URL` variable overrides `base_url`, and zendesk publishes
      the `unbound-base-url-template` issue for exactly this. A test asserts no shipped provider has
      an unbound template variable, and the issue code disappears for a provider that binds its own.
- [ ] Multiple servers per service where the vendor has them — production versus sandbox, and AWS's
      per-region hosts — with one declared as default and the selection being operator config, never
      a compiled-in choice. `providers/babelforce.toml`'s header records why: its document's
      `servers[0]` is *staging*, so a positional "take the first server" ingest would have pointed the
      connector at dev.
- [ ] `http_hosts` derives from the endpoint spec across every declared server and is never widened
      to `*` (C-10).
- [ ] No credential ever enters an endpoint spec — a URL with an embedded token is a credential, and a
      test refuses it.

## Progress
- Not started. Filed 2026-07-30 while answering "what else could a connector carry".

## Notes
- **Sequence after [C-49](C-49-provider-services.md)**, which moves `base_url` onto the service and
  makes per-service endpoints structural — AWS is the case that forces it.
- This is the least speculative item on the list: the gap is recorded in three provider files and
  published as a per-operation issue in the catalogue today, so it is a known defect rather than a
  new capability.
