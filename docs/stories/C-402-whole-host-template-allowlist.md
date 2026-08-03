---
id: C-402
title: "Decide whether a whole-host template needs an operator allowlist"
pillar: Bridge
status: ready
priority: 4
note: "DECIDED 2026-08-01, refined by C-508: the connector declares a closed host bound or an explicit operator-pinned self-managed-origin policy; the loader REFUSES a whole-host template declaring neither"
---

# Decide whether a whole-host template needs an operator allowlist

## Goal

Decide, and record, whether a connector whose `base_url` templates the **whole** authority should
require something more than "the value is a hostname" before a tenant's configuration can compose it.

## The finding this comes from

Measured 2026-08-01. Thirteen base URLs carry a `{placeholder}`, in three shapes:

| shape | connectors | what a config value can do |
|---|---|---|
| label inside a fixed suffix | zendesk, jira, confluence, shopify, salesforce, supabase, mailchimp | cannot leave the vendor: the suffix is template-supplied and `Slot::Host` refuses any value that would escape it |
| **whole authority** | **freshdesk, newrelic, okta, docusign** | **is** the host — `validate_authority` accepts `127.0.0.1`, `localhost`, `169.254.169.254` |
| path segment only | contentful, statuspage | cannot touch the authority at all |

`Slot::Host` is **not** defective: its stated job is refusing a value that escapes the authority
(`@`, `:`, `/`, `%`, whitespace, control, empty label), and it does that in all three shapes. What was
wrong was `crates/connectors-api/README.md`, which summarised it as *"no shipped connector can be
pointed at a loopback address"* and offered that as a second, independent layer beside the SSRF
guard. For the four whole-host connectors there is one layer, not two.

Pinned by `connector-pack`'s `a_whole_host_template_is_constrained_only_to_being_a_hostname`.

## Acceptance

- [x] A decision, recorded below, among at least these:
      1. **Nothing changes.** The SSRF guard is the layer, the tenant supplies its own host for its
         own credential, and a tenant choosing where its own secret goes is not an escalation.
         Defensible — but it must be *chosen*, and the asymmetry with the suffix-shaped connectors
         written down.
      2. **The connector declares a host allowlist/suffix or an operator-pinned self-managed-origin
         policy**, and the loader refuses a whole-host template that declares neither. Strongest,
         and it costs a provider-TOML field.
      3. **The host refuses a whole-host template unless the operator supplies an allowlist**, which
         puts the decision at deployment rather than in the catalogue.
- [ ] If the decision is (1), the reasoning lands next to the guard, not only in a story.
- [ ] If the decision is (2) or (3), a **failing-first test** shows a whole-host connector refusing a
      value outside its declared or operator-approved bound, and the four affected connectors are
      migrated.
- [ ] Either way, the multi-tenant case is addressed explicitly: whether tenant A supplying a host
      for its own connection can affect tenant B, and why not.

## The decision (2026-08-01)

**Option (b), refined for self-managed products: the connector declares its authority bound, and
the loader refuses a whole-host template that declares none.** A vendor with a finite SaaS surface
declares an allowlist or suffix. A connector for a self-managed product may instead declare that its
HTTPS origin is an operator/deployment pin. That pin is configuration metadata and policy, not an
operation argument: an ordinary Service Account or model invocation cannot choose or widen it.

Derived from this repository's own principles rather than from taste:

- *"A connector declares what it needs, and nothing grants itself access."* A connector that templates
  its entire authority is asking for an unbounded grant and currently says nothing about it. Making it
  declare either a vendor bound or an operator-pin requirement is the same move the credential model
  already makes.
- *"The vendor spec is the source of truth; drift is detected, not absorbed."* Which hosts a SaaS
  vendor serves is a **vendor fact**, so its closed bound belongs in the provider definition. A
  self-managed installation's origin is instead a deployment fact; the provider must declare that
  policy explicitly so it cannot be mistaken for an unconstrained connection value.
- *"Refuse; never repair."* A loader that accepts a whole-host template with no bound is absorbing the
  gap rather than detecting it.

**Why not the others.** (a) is passive: it leaves the asymmetry between seven suffix-shaped connectors
and four unbounded ones undocumented and unchecked, which is how the README came to claim a layer that
was not there. (c) lets a host invent a restriction without the connector declaring that it needs one,
so the guarantee varies silently by deployment — precisely the property Exchange cannot build on when
it promises callers "cannot name a host". The self-managed refinement does not do that: the connector
publishes the operator-pin requirement as part of its contract; only the installation-specific value
comes from deployment policy.

## Progress
- 2026-08-03: C-508 refined the decision for self-managed products. GitLab will be the reference
  operator-pinned HTTPS-origin case; this story retains the fleet migration for the four existing
  unconstrained whole-authority templates.
- Decision recorded. Implementation not started.

## Notes
- Worth weighing before it feels urgent: the threat is not obviously a tenant misdirecting **its
  own** credential — that is theirs to do. It is (a) SSRF from a shared host, which the egress guard
  already answers, and (b) a *stored* connection whose host was set once and is trusted later by
  something that assumes the vendor.
- This matters beyond this repository: flux-exchange plans a credential store on the same model, and
  its charter promises the caller "cannot name a host". For seven connectors that is true because of
  the template; for four it is true only because of the egress guard. The exchange should know which.
