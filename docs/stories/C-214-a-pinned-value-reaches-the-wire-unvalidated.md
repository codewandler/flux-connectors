---
id: C-214
title: "An operator-supplied configuration value reaches the URL unvalidated — and in a host position it can move the origin"
pillar: Bridge
status: ready
priority: 1
design: docs/designs/connector-configuration.md
epic:
areas: [bridge, connector-pack]
note: "found by C-187's review 2026-07-31 and reproduced against the shipped catalogue. The path/query half is a wrong endpoint at the right vendor; the `endpoint.<var>` host half is PRE-EXISTING and changes the authority — `subdomain=\"acme.zendesk.com@evil.example\"` resolves to evil.example.zendesk.com"
---

# An operator-supplied configuration value reaches the URL unvalidated

## Goal

Validate a configuration value where it is substituted, not only where it is declared — so that the
guard the loader already implements actually runs against the value that travels.

## What was measured

`connector-spec` has the guard. `Position::validate_value` exists and is correct. It has exactly two
non-test call sites, **both in the loader**, and both run against an `example` or a parameter *name*:

```
crates/connector-spec/src/provider.rs:686   position.validate_value(example)
crates/connector-spec/src/provider.rs:708   position.validate_value(pinned)   // the NAME, not the value
```

The real value is substituted at `crates/connector-pack/src/request.rs:484` (`Node::Lit` →
`self.substitute(literal)`, calling `substitute` at `:519`) with no predicate at all. Probed against
the shipped catalogue:

```
zone_id="../../v4/other"       -> https://api.cloudflare.com/client/v4/zones/../../v4/other/dns_records
zone_id="x/../../y"            -> https://api.cloudflare.com/client/v4/zones/x/../../y/dns_records
zone_id="abc?evil=1"           -> https://api.cloudflare.com/client/v4/zones/abc?evil=1/dns_records
zone_id="abc#frag"             -> https://api.cloudflare.com/client/v4/zones/abc#frag/dns_records
zone_id="abc%2Fdef"            -> https://api.cloudflare.com/client/v4/zones/abc%2Fdef/dns_records
zone_id="abc\ndef"             -> https://api.cloudflare.com/client/v4/zones/abc\ndef/dns_records
teamId="team_a&projectId=evil" -> https://api.vercel.com/v10/projects?teamId=team_a&projectId=evil
```

## The severe half is the pre-existing one

**A path or query pin cannot change the origin.** Substitution lands after the authority is fixed in
the `base` literal, so `url::Url::parse` keeps the vendor's host in every case above. `..` normalises
to a different path, `?`/`#` truncate it, the newline is stripped. The outcome is a **wrong endpoint
at the right vendor, carrying the operator's own token** — bad, bounded.

**A host-position `endpoint.<var>` is a different matter, and it predates [C-187](C-187-config-cannot-pin-a-request-component.md):**

```
subdomain = "acme.zendesk.com@evil.example"
  -> https://acme.zendesk.com@evil.example.zendesk.com/api/v2/tickets/1.json
     authority: evil.example.zendesk.com
```

The `@` makes everything before it userinfo, so the request goes to a host the operator did not
name. Nine shipped connectors carry a templated host (`zendesk`, `shopify`, `jira`, `freshdesk`,
`salesforce`, `docusign`, `okta`, `contentful`, `statuspage`). **This is the half to fix first.**

## Why it is not simply "escape the value"

Percent-encoding a path segment is right; percent-encoding a *host* is not — a host has different
legal syntax and a different failure mode. The three positions need three answers:

- **Host:** the resolved authority must equal the authority the declaration implies. Comparing the
  parsed host against the template's fixed suffix is stronger than blocklisting `@`, `/` and `:`,
  because it fails closed on the cases nobody enumerated.
- **Path segment:** percent-encode, or refuse a value containing a reserved character. Refusing is
  probably better — a `zone_id` with a slash in it is an operator mistake, and silently encoding it
  produces a 404 they cannot diagnose.
- **Query value:** `auth::query_encode` already exists and is the identity over unreserved
  characters. Reuse it rather than writing a second encoder.

## Acceptance

- [ ] **Failing-first test:** the host case. A `subdomain` of `acme.zendesk.com@evil.example` must not
      produce a request whose authority is `evil.example.zendesk.com`. It fails today. Name it.
- [ ] A path-position value containing `/`, `..`, `?`, `#`, `%2F` or a control character is refused or
      encoded — decide which per position and record the reason. A refusal must name the field, the
      operation and what is wrong with the value.
- [ ] A query-position value goes through the existing `auth::query_encode` rather than a second
      encoder.
- [ ] **Whitespace-only values are covered.** `" "` currently survives the empty-string filter at
      `crates/connector-pack/src/config.rs:278` and reaches the wire as `?teamId=%20`. An
      all-whitespace configuration value is not a value.
- [ ] A raw newline cannot reach a **header** pin. No shipped provider declares one yet
      ([C-164](C-164-provider-algolia.md) will be the first), so this must be proved against a fixture
      rather than the catalogue — header injection is the one position here with a classic exploit.
- [ ] The validation runs at **substitution time** in `connector-pack`, so it binds every host and
      every `ConfigStore`, not only the loader's view of an `example`.
- [ ] `connector-spec`'s `Position::validate_value` is reused rather than reimplemented, or is
      deliberately replaced with the reason recorded. Two spellings of one rule is the defect this
      story is already an instance of.

## Notes

- **Severity, stated plainly so it is neither overplayed nor dismissed:** the value is
  operator-supplied, not attacker-supplied, so this is not a classic injection. It is a
  paste-the-wrong-thing hazard, and for the host case the wrong thing goes somewhere the operator
  never named. With [C-204](C-204-google-signin-accounts.md) landing multi-account sign-in, "the
  operator" is no longer necessarily the person who owns the deployment.
- **Open question worth settling first**, from C-187's review: does any consuming host's egress
  allow-list match on prefix or path rather than host? `Operation::subjects`
  (`crates/connector-pack/src/tool.rs:354-364`) hands out the raw, **un-normalised** `request.url`
  while the wire carries the same string — so if a matcher normalises `..` and the subject check does
  not, the two diverge and this becomes a gate bypass rather than a wrong endpoint. That matcher
  lives outside this repository.
- The `connectors-api` host configures `PrivateNetAllow::None`, so the SSRF guard still refuses
  private and loopback destinations. It does not refuse a *public* host the operator did not intend.
