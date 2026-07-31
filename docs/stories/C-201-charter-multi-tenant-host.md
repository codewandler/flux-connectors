---
id: C-201
title: "Amend the charter: a deployed multi-tenant host is in scope"
pillar: Bridge
status: ready
priority: 1
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, docs]
note: "owner-directed 2026-07-31. vision.md's non-goal and connectors-app.md's loopback narrowing both forbid what C-200 builds; without this the repository's own contract reads as violated"
---

# Amend the charter: a deployed multi-tenant host is in scope

## Goal

Make [C-200](C-200-connectors-api-epic.md) legal in the repository's own terms, and redo the analysis
that the narrowing was standing in for — so the service ships with its risks named rather than with
its charter quietly out of date.

## Why this is a story and not a paragraph in a PR

Three documents currently forbid this service, each for a stated reason:

- `docs/vision.md` — *"A runtime for production traffic… Loopback-bound, never published, never a
  production request path."*
- `docs/designs/connectors-app.md` — the narrowing table: callers are *"the operator sitting in front
  of it"*, credential scope is *"one operator's own, in one process they started"*, binding is
  *"loopback only, no configuration to change it"*, and *"the first PR that adds a `--bind` flag is
  the one to refuse."*
- `docs/designs/connectors-proxy.md` — *"a credential-injecting proxy is, by construction, a
  confused-deputy machine: its entire job is to add authority a caller does not have."*

The third is not a preference. It is the argument C-34 resolved, and a multi-tenant service reopens
it in full: the service holds many tenants' credentials and adds authority to a caller who does not
hold them. Amending the first two without answering the third would delete the analysis rather than
address it.

## Acceptance

- [ ] `docs/vision.md`'s non-goal is amended to permit a deployed multi-tenant host, naming what is
      still out of scope. The amendment states what changed and who directed it.
- [ ] `docs/designs/connectors-app.md` is marked **superseded by** `connectors-api.md`, not deleted.
      Its slice-1 sequence and its `Egress`/transport analysis remain correct and are cited by C-202
      and C-203.
- [ ] `docs/designs/connectors-api.md` carries a **Confused deputy** section that answers
      `connectors-proxy.md` directly: who the principal is, what proves a caller may use a tenant's
      credential, what stops tenant A's session reaching tenant B's secret, and what the service
      refuses to do. "It is authenticated" is not an answer on its own — the proxy design already
      rejected that as the mitigation.
- [ ] `AGENTS.md`'s **Intentional gaps** entry for "nothing here makes a live call, because nothing
      here is a host" is updated rather than left standing, since it stops being true.
- [ ] No document in the repository still asserts the service may not exist. Checked by grep for
      `loopback`, `never published`, and `production request path`.

## Notes

- Keep `publish = false` on the service crate regardless. The amendment is about *deployment*, not
  about putting a host on crates.io; the publish closure stays four crates
  ([C-190](C-190-publish-catalog-pack-secrets.md)).
- The narrowing that should survive verbatim: **the service constructs no request of its own.** That
  was the structural reason `connectors-app` superseded `connectors-proxy`, and it is unaffected by
  tenancy.
