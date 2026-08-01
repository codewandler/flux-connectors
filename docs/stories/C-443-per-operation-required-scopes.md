---
id: C-443
title: "An operation should declare the scope it requires, so a host can request least privilege"
pillar: Spec
status: backlog
design: docs/designs/unified-auth.md
epic: connectors-v1
areas: [connector-spec, providers]
note: "the idea is right and the prerequisite is missing: NO provider declares an [auth.oauth2] block, so there is no acquisition to minimize, and the one spec-backed vendor's per-operation scope is literally '*'. Blocked on C-440"
---

# An operation should declare the scope it requires, so a host can request least privilege

## Goal

Let an `[[operations]]` entry declare the OAuth scope(s) it requires, and derive the connector's and
each service's **requested** scope set as the union — so a host installing nine of forty operations
asks the vendor for the nine operations' scopes rather than for everything.

## Why this is worth doing, stated as the consumer rather than as the field

The weak version of this idea is documentation: publish scopes so a reader can see them. That is not
worth a field in the hash domain.

The strong version is **least privilege at install time**, and it has a concrete mechanism. A host
that knows which operations a tenant selected can compute the minimal scope set to request. That
also makes the connector-level requested set *derived* rather than authored — the rule
[`Role`](C-120-service-roles-declaration.md) (C-120), `Tag` ([C-153](C-153-service-tags.md)) and
`Level` all already follow, and the rule this repository reaches for whenever a value could be stated
in two places and disagree with itself.

`AuthMethod::scopes` (`crates/connector-spec/src/auth.rs:189-191`, *"Requested scopes"*) is the field
that union would populate. Today it is authored, and nobody authors it.

## Measured, 2026-08-02 — why this is `backlog` and not `ready`

Every line here is a command's output from the session that filed the story.

- **No shipped provider declares an `[auth.oauth2]` block.** Not one of the 54. Several record the
  reason in prose rather than leaving it silent — `providers/airtable.toml:228`,
  `providers/intercom.toml:197`, `providers/notion.toml:199`, `providers/docusign.toml:163`,
  `providers/statuspage.toml:35`: the token is minted at install time by an OAuth app, and flux is
  handed one already minted. **So there is no acquisition for a scope set to minimize.** A derived
  union with no consumer is a field, not a feature.
- **`AuthMethod::scopes` is declared by zero providers** — `grep -n "scopes\? = " providers/*.toml`
  returns nothing. The field the union would feed is itself unused.
- **The one spec-backed provider carries no usable scope data.** `providers/babelforce.toml` is the
  only file with a `[spec]` front-end. Its `task-automation` document has 31 per-operation
  `security:` blocks and every one of them reads:

  ```yaml
  security:
    - bearerAuth: []
    - oauth2:
        - '*'
  ```

  and the scheme declares exactly one scope — `'*': All Access`. Ingesting that would publish
  `scopes = ["*"]` on 31 operations: a field that looks like a permission contract and discriminates
  nothing.
- **The other vendored documents declare no security at all.**
  `specs/zendesk/2024-06-01-excerpt.json` and `specs/anthropic/2023-06-01-excerpt.yaml` have zero
  `security:` occurrences.

So the honest answer to "store it if available" is that **today it is not available** — not from the
spec route, because only babelforce takes it and its answer is `*`; and not from the hand-authored
route without inventing it, which is what [C-126](C-126-response-schema-coverage.md) refuses.

## The prerequisite

[C-440](C-440-declare-an-acquisition-and-its-hazard.md) — *"An `[[auth]]` block can declare an
acquisition the host performs, and the hazard it carries"*. Once a connector can declare an
acquisition, there is something to minimize and this story has a consumer. Until then it is a field
with neither a producer nor a consumer, which is the shape `AGENTS.md` §Intentional gaps calls out
for `quirks.rate_limit` — *"not an unfinished feature; it is a shape the model does not need"*.

**Do not close this by ingesting babelforce's `'*'`.** That would satisfy the letter of "store the
scope if the spec has one" and put a meaningless value in the hash domain and in every artifact.

## Acceptance

*(To be sharpened when C-440 lands and a real acquisition exists to size this against.)*

- [ ] An `[[operations]]` entry may declare `scopes = [...]`, and the loader refuses a scope that the
      connector's declared acquisition does not know about — the check is what makes the declaration
      worth anything, exactly as it is for a role.
- [ ] A service's and the connector's requested scope sets are **derived** as the union of their
      operations', never authored. An authored `scopes` at those levels is a load error.
- [ ] Scopes are only declared where the **vendor documents them per endpoint**. A vendor that
      publishes one all-access scope declares none here, and the provider file says so in a comment
      rather than leaving the absence unexplained.
- [ ] A host can ask: given this set of operations, what is the minimal scope set to request.
- [ ] **Failing-first test:** an operation declaring a scope outside the connector's acquisition is
      refused, naming the scopes that exist.
- [ ] The gate is green and the build stays a fixed point.

## Progress
- (not started — `backlog` by the measurement above, not by priority)

## Notes
- Filed from a direct question: *"should we store required scope per operation in the catalog if
  available — probably"*. The instinct is right; the corpus is not ready for it yet, and this file
  exists so the next person to ask does not have to re-measure.
- Scopes would be the **fourth** declared-per-member/derived-above field, after `roles`, `tags` and
  `Level`. If a third of these lands, the derivation deserves one helper rather than four unions —
  [C-153](C-153-service-tags.md)'s Notes already say this about the second and third.
- Related but distinct: this is about what an operation *needs*. `[[auth]]`'s `credentials` says which
  credential an operation uses, which is a different question and already modelled.
- If it lands, it interacts with [C-442](C-442-tags-and-roles-reach-an-artifact.md): a scope set that
  reaches no artifact helps no host, and C-442 is where the surface-to-artifact projection is decided.
