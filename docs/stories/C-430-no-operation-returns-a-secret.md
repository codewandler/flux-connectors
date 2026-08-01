---
id: C-430
title: "No operation returns a secret — and three shipped in v0.9.0 that do"
pillar: Spec
status: ready
priority: 1
areas: [providers, connector-spec, connector-cli]
note: "owner-stated 2026-08-01: 'we just cannot use ops which return secrets, so no refresh, no token exposed as ops'. Found by a catalogue-wide scan the same day: zoom-meeting-create and zoom-meeting-get return start_url, which their own description calls HOST-PRIVILEGED and says to treat as a credential; babelforce-get-user-customer returns accessToken. All three are live in v0.9.0"
---

# No operation returns a secret — and three shipped in v0.9.0 that do

## Goal
Make "an operation's response must not carry a secret" a rule the build enforces, and remove the
three operations currently violating it.

## The finding

`AGENTS.md` § Authentication contract now states the rule — an operation whose declared response
carries a token is withheld, because the host's redactor holds only values the host itself resolved
and cannot know a secret minted by the very call returning it. It was written for `/oauth/token`.
A scan of all **681 operations across 53 providers** in `web/public/catalog.json` shows it was never
only an OAuth problem:

| Operation | Field | The vendor's own words |
|---|---|---|
| `zoom-meeting-create` | `start_url` | *"HOST-PRIVILEGED. Embeds the host's ZAK token: anyone holding this URL starts the meeting as its host. Treat it as a credential: do not log it, echo it…"* |
| `zoom-meeting-get` | `start_url` | as above |
| `babelforce-get-user-customer` | `accessToken` | described in the document as *"The unique Identifier (UUID) of the object"* — the description is generic and the field name is not |

**The Zoom pair is the one that matters, and it is not a new discovery** —
[C-79](C-79-sensitive-response-fields.md) has carried *"Zoom's `start_url` carries a host-privileged
token · the redactor cannot see it"* in its frontmatter since it was filed, and it is still `ready`.
The connector documents the hazard accurately and then returns the field anyway. Describing a
credential is not withholding it.

The scan also produced 28 false positives, and they are worth recording so the gate does not chase
them: babelforce's `sessionId`/`session_id` (a call-session identifier), Klaviyo's `public_api_key`
(*"Public by design — it is embedded in the account's own web pages"*), Typeform's `token` (*"This
response's own opaque id"*) and Zendesk's `authenticity_token` (*"not a credential for this API"*).
Every one of those is correctly documented in the connector. **A name-shaped heuristic is not the
rule** — the rule is about what the value *is*.

## Acceptance
- [ ] The three operations no longer ship, each recorded as a named exclusion with its reason —
      the same three-category accounting babelforce already uses (emitted / inexpressible / withheld).
- [ ] **A gate fails the build when an operation's declared response carries a credential**, so this
      cannot recur silently as connectors widen. A failing-first test reinstates one of the three and
      asserts the build refuses.
- [ ] **The gate is declaration-driven, not name-matching.** 28 of 31 scan hits were false positives
      whose connectors already document them as harmless. The mechanism [C-79](C-79-sensitive-response-fields.md)
      designs — a connector *declaring* that a response field is a credential — is what the gate reads.
      A regex over field names would fail every one of those four and teach authors to fight it.
- [ ] `docs/designs/spec-front-end.md` and `AGENTS.md` agree on one statement of the rule; the
      Authentication contract already carries it and must not be restated differently.

## Progress
- (not started)

## Notes
- **Sequenced with [C-79](C-79-sensitive-response-fields.md), which owns the declaration** this
  gate reads, and with [C-136](C-136-credential-diversion.md), which owns the eventual answer: an
  operation that legitimately produces a credential returns a **handle**, not the secret. Until C-136
  lands, withholding is the only available answer, and this story is that.
- The Zoom pair is a **release regression in the honest sense**: they shipped in v0.9.0 and in every
  release before it. Removing them narrows a published catalogue, which is a user-visible change and
  wants a `WHATS-NEW.md` entry saying plainly what stopped being available and why.
- Scan command used, so the next person reproduces rather than re-derives it: walk every
  `providers[].operations[].response_schema` in `web/public/catalog.json` for property names matching
  token/secret/password/api_key/private_key/credential/refresh/start_url, then **read each hit's
  description** — that second step is what separated 3 from 31.
