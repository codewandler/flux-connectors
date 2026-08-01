---
id: C-402
title: "Decide whether a whole-host template needs an operator allowlist"
pillar: Bridge
status: ready
priority: 4
note: "four connectors template the ENTIRE authority, so Slot::Host constrains the value only to being a hostname — 127.0.0.1 and 169.254.169.254 both compose. Not a defect; a thinner layer than the docs claimed"
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

- [ ] A decision, recorded in a design note, among at least these:
      1. **Nothing changes.** The SSRF guard is the layer, the tenant supplies its own host for its
         own credential, and a tenant choosing where its own secret goes is not an escalation.
         Defensible — but it must be *chosen*, and the asymmetry with the suffix-shaped connectors
         written down.
      2. **The connector declares a host allowlist or suffix**, and the loader refuses a whole-host
         template that declares neither. Strongest, and it costs a provider-TOML field.
      3. **The host refuses a whole-host template unless the operator supplies an allowlist**, which
         puts the decision at deployment rather than in the catalogue.
- [ ] If the decision is (1), the reasoning lands next to the guard, not only in a story.
- [ ] If the decision is (2) or (3), a **failing-first test** shows a whole-host connector refusing a
      value outside its allowlist, and the four affected connectors are migrated.
- [ ] Either way, the multi-tenant case is addressed explicitly: whether tenant A supplying a host
      for its own connection can affect tenant B, and why not.

## Progress
- (not started)

## Notes
- Worth weighing before it feels urgent: the threat is not obviously a tenant misdirecting **its
  own** credential — that is theirs to do. It is (a) SSRF from a shared host, which the egress guard
  already answers, and (b) a *stored* connection whose host was set once and is trusted later by
  something that assumes the vendor.
- This matters beyond this repository: flux-exchange plans a credential store on the same model, and
  its charter promises the caller "cannot name a host". For seven connectors that is true because of
  the template; for four it is true only because of the egress guard. The exchange should know which.
