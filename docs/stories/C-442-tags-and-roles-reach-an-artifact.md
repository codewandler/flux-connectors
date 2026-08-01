---
id: C-442
title: "A service's tags and roles reach no artifact, so nothing can filter or resolve them"
pillar: Codegen
status: ready
priority: 2
design: docs/designs/connector-surfaces.md
epic: provider-roles
areas: [connector-cli, catalog, web]
note: "split from C-153, which shipped the declaration and could not ship the consumer. roles has had the same hole since C-120 — do BOTH in one projection, or the second one duplicates the first"
---

# A service's tags and roles reach no artifact, so nothing can filter or resolve them

## Goal

Two declared surfaces stop at the IR. [C-120](C-120-service-roles-declaration.md) landed `roles` and
[C-153](C-153-service-tags.md) landed `tags`; the loader checks both, the hash domain covers both,
and **neither reaches a single byte of output**. Project them into the manifest and `catalog.json`,
give `catalog` a way to query them, and let the explorer filter by tag.

## Measured, 2026-08-02

- `web/public/catalog.json`'s service objects carry exactly
  `api_version, base_url, description, gid, hosts, name, operation_count`. **No `roles`, no `tags`.**
- `connectors/anthropic-models.connector.toml` carries no `roles` either — `grep -n roles` finds
  nothing — even though `providers/anthropic.toml` has declared `roles = ["llm_catalogue"]` since
  C-120.
- After C-153 tagged all 54 providers, `cargo run -p connector-cli -- build` reported **`1 written`**
  — `connectors.lock`. Fifty-four providers gained a declared fact and the artifacts did not move.

That last line is the whole story: a field in the hash domain that changes zero artifact bytes.
`AGENTS.md` §Intentional gaps lists `[[services]] roles` in its six-dead-surfaces table, and
`docs/designs/connector-surfaces.md:226` assigns the roles projection to
[C-121](C-121-llm-catalogue-role.md).

## Do both in one pass, and that is the point of this story

C-153 deliberately did **not** build a tags-only path. Two reasons, and they are why this story
exists rather than a narrower one:

1. **`AGENTS.md` forbids the ad-hoc version** — *"Do not close this by widening the manifest ad hoc;
   the surface-to-artifact mapping is decided in `connector-surfaces.md`."*
2. A tag-only projection leaves `roles` dead beside it, and whoever lands C-121 then writes the same
   plumbing a second time and has to reconcile two shapes.

**Coordinate with [C-121](C-121-llm-catalogue-role.md)**, which owns the roles half by
`connector-surfaces.md`'s own assignment. Either this story subsumes that half — in which case say so
in C-121 and narrow it to the vocabulary work — or C-121 lands first and this one consumes its
projection. Do not run them in the same wave; they write the same emitters.

## Acceptance

- [ ] A service's `roles` and `tags` reach `connectors/<name>.connector.toml` and
      `web/public/catalog.json`, under whatever the every-key-always-present rule
      (`docs/designs/catalog-json.md`) requires — a consumer must not have to distinguish "absent"
      from "empty".
- [ ] The **provider-level derived unions** (`Connector::roles`, `Connector::tags`) reach the
      catalogue too, since that is the level a UI filters at.
- [ ] `crates/catalog` gains a query for "which services carry this tag" and "which carry this role".
- [ ] The explorer filters by tag, and **renders a tag distinguishably from a role** — the misread
      C-153 names is that a category implies a capability, and `SpecChip`'s tone is derived from the
      value, so a tag must not borrow a tone that reads as a safety claim.
- [ ] **The `SCHEMA_VERSION` decision is made deliberately, not by accident.**
      `connector-surfaces.md` flags that the sibling config projection ([C-87](C-87-configuration-codegen.md))
      carries a breaking schema-version bump; adding two keys to every service object may too. State
      which it is and why.
- [ ] **Failing-first test:** a test asserting `anthropic`'s `models` service publishes
      `llm_catalogue`, and that a tagged service publishes its tags — failing at the merge base
      because the keys do not exist.
- [ ] The build is a fixed point afterwards and `connector-cli -- diff` shows only the intended
      additions.
- [ ] The gate is green.

## Progress
- (not started)

## Notes
- Split from [C-153](C-153-service-tags.md); read its Progress for what landed and what the 47
  single-surface providers' `default` entry cost.
- Four of the six dead surfaces in `AGENTS.md`'s table are the sharp ones. This story takes one of
  them (`roles`) and the field C-153 just added beside it; `config` and `verify` stay with C-87.
- `crates/connector-cli/src/site.rs` builds the catalogue objects; that is where the service shape is
  decided today.
