---
id: C-37
title: Give providers and operations stable global addresses
pillar: Spec
status: ready
priority: 5
design: docs/designs/global-addressing.md
epic: connectors-v1
areas: [connector-spec, connector-cli, providers]
note: pid / gid / oip · the global half; C-23 stays the local half
---

# Give providers and operations stable global addresses

## Goal
Give every provider and operation an identifier that is unique outside this repo, carries the vendor
and the API version, and stays stable across regeneration — so docs, the lockfile and any external
consumer can reference an operation by address rather than by a local symbol.

## Acceptance
- [ ] Three levels render and parse:
      `pid` = `com.zendesk.api` ·
      `gid` = `com.zendesk.api/support/tickets:v2` ·
      `oip` = `com.zendesk.api/support/tickets:v2#show`
- [ ] **Structured fields, not an authored string.** `Connector` gains `authority` and `api_version`;
      `Operation` gains `path: Vec<String>`, `operation: String`, and an optional `api_version`
      override. The oip string is *rendered* from them.
- [ ] `parse(render(x)) == x` round-trips, tested with C-2's discipline.
- [ ] Validation: authority is a reverse-DNS label sequence; segments and operation are lowercase
      kebab; version matches the vendor's spelling. **oips unique within a connector, pids unique
      across `providers/`** — a collision is a loud error, as C-3 already treats duplicate op ids.
- [ ] Golden error snapshots for a malformed oip and for a collision, following the existing corpus
      in `crates/connector-spec/tests/golden/`.
- [ ] New fields land **inside** `HashDomain::of` — they are part of a connector's compiled meaning,
      unlike provenance. C-2's determinism tests stay green unchanged.
- [ ] `<provider>.connector.toml` carries the pid and a per-operation oip; `connectors.lock` keys
      entries by pid.
- [ ] All three `providers/*.toml` declare their addresses, and `cargo run -p connector-cli -- build`
      still writes all three modules.
- [ ] `AGENTS.md` records the stability contract: **an oip, once published, is never reused for a
      different operation.** A rename mints a new oip and deprecates the old.

## Progress
- (not started)

## Notes
- **`Operation.id` is unchanged and stays the declarable Flux symbol.** C-8 proved flux's `decl_name`
  grammar admits only alphanumerics, `_` and `-`, so an oip can never be a Flux declaration name.
  That is precisely why there are two identifiers rather than one richer one. The four goldens in
  `crates/connector-flux/tests/golden/` and every provider TOML already pin the local symbols;
  churning them would be a large diff for no gain.
- **Version is the vendor's API version**, not ours, so the oip is stable across our regenerations and
  two connectors for v1 and v2 can coexist. Our connector version stays in `connectors.lock`.
- **Variable path depth is deliberate:** freshdesk has one segment (`com.freshdesk.api/tickets:v2`)
  where zendesk and babelforce have two. A positional scheme would need an empty slot; `/` hierarchy
  does not.
- **`#` friction is accepted, with three mitigations that must hold:** an oip is never a TOML *key*
  (it is rendered, not authored), generated docs always quote it in shell examples, and proxy routes
  (C-35) address by path segments rather than embedding a raw oip in a URL.
- **Sequencing: lands after C-29.** C-29 is the last blocker on any `.flux` being generated at all;
  this is additive metadata and generation working is worth more.
- **Read [C-49](C-49-provider-services.md) before starting.** It promotes this design's middle level
  from anonymous `path` segments to a named `Service` that owns the version and the base URL, so
  `Operation.path: Vec<String>` and `Connector.api_version` are the fields it reshapes. Landing this
  story first publishes an address scheme that C-49 then changes — against this story's own stability
  contract. Either take C-49 first or land the two together.
- The oip is what C-31's docs pages use as each operation's canonical heading and anchor.
