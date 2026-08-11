---
id: C-523
title: "Publish one canonical normalized HTTPS-origin API for every consumer"
pillar: Bridge
status: ready
priority: 0
design: docs/designs/connector-configuration.md
epic: connector-config
areas: [connector-address, connector-spec, connector-pack, tests]
note: "Milestone 1 blocker: Exchange X-125 must validate and compare the exact origin contract without copying connector-spec or depending on an unpublished compiler crate"
---

# Publish one canonical normalized HTTPS-origin API for every consumer

## Goal

Make an operator-approved HTTPS origin one published, normalized value type that the connector
compiler, connector runtime pack and Exchange can all consume without copying a parser.

C-508 left two production validators: `connector-spec` owns the declaration-time spelling while
`connector-pack` owns a runtime copy because the compiler is intentionally unpublished. Exchange
X-125 now needs the same rule before it can persist, compare and approve a proposal. A third copy
would let one spelling be approved while another is executed, and depending on `connector-spec`
would publish compiler machinery merely to share a small value vocabulary.

## Acceptance

- [ ] Published `codewandler-connector-address` owns a public `HttpsOrigin` value type and a closed,
      typed refusal. The API is pure, performs no IO, adds no parser or compiler dependency, and its
      public signature exposes only standard-library and connector-address-owned types.
- [ ] Parsing returns a normalized value rather than a validated input string. Its canonical text
      spells the scheme `https`, lowercases ASCII DNS names, renders IPv4 and bracketed IPv6 through
      their canonical standard-library spelling, renders a non-default port as canonical decimal,
      and omits the effective default `:443` port. `Eq`, ordering and hashing therefore compare
      origins, not caller spelling.
- [ ] Equivalent safe input spellings normalize to one value, including case-insensitive HTTPS and
      DNS spelling, IPv6 compression and a zero-padded/default port. Userinfo, HTTP, an empty or
      non-ASCII/invalid host, an unbracketed IPv6 address, a zero/out-of-range port, path (including
      `/`), query, fragment, whitespace and placeholder braces remain refusals. The connector owns
      every byte after the origin.
- [ ] The refusal is value-free by construction: variants identify the rejected class without
      retaining the supplied text, and `Display`, `Debug`, error chaining and tests never reproduce
      the configured origin. The normalized value also does not expose its customer-supplied text
      through an incidental derived `Debug`; deliberate canonical rendering remains an explicit API.
- [ ] One committed corpus carries input, expected canonical output or expected refusal variant.
      `connector-address`, `connector-spec`'s `Format::Origin`/loader path and `connector-pack`'s
      projection/runtime path all consume that corpus; the old spec-owned table and both copied
      production validators are removed.
- [ ] Connector declarations publish canonical defaults and examples: the loader refuses a
      declaration whose accepted origin is not already in canonical form. Runtime connection input
      may use an equivalent safe spelling and is normalized before approval/default comparison,
      request composition, permission subjects, intents or evidence are derived.
- [ ] `connector-pack` depends directly on `connector-address`, never on `connector-spec`, and uses
      the same normalized `HttpsOrigin` instance for the request destination and the permission
      subject. GitLab's canonical default stays byte-for-byte `https://gitlab.com`; an equivalent
      spelling of that default does not become a custom-origin approval event.
- [ ] The crates.io publish-closure and packaging tests prove `connector-address` and
      `connector-pack` are consumable together while `connector-spec`, `connector-flux` and
      `connector-cli` remain unpublished compiler machinery. No path or git dependency is added.
- [ ] The public API documentation gives Exchange one registry dependency and one parse/normalize
      contract. Exchange needs no origin parser, provider-TOML reader, sibling checkout, path/git
      dependency or `connector-spec` publication to implement X-125.
- [ ] Failing-first tests cover canonicalization, typed/value-free refusals, compiler/runtime corpus
      parity, GitLab default/custom approval equivalence and request/permission-subject identity;
      the targeted crate tests and publish-closure/package gates are green.

## Progress

- (not started)

## Notes

- This is the corrective prerequisite for Exchange X-125. It does not move approval state into the
  connector library: Exchange still owns tenant/connection revisions and the Proposed → Approved →
  Revoked lifecycle. The shared type answers only whether two safe origin spellings name the same
  normalized destination.
- `connector-address` already sits below `connector-spec` and in the published closure. Extending
  that small vocabulary preserves C-407's dependency direction; making `connector-spec` publishable
  would reverse the decision and is explicitly not an alternative.
- Releasing the API follows the ordinary connector release pipeline. Exchange consumes the released
  crate from crates.io; a sibling path dependency is not temporary integration evidence.
