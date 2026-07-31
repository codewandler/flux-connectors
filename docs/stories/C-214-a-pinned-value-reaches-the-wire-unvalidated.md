---
id: C-214
title: "An operator-supplied configuration value reaches the URL unvalidated — and in a host position it can move the origin"
pillar: Bridge
status: done
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

- [x] **Failing-first test:** the host case. A `subdomain` of `acme.zendesk.com@evil.example` must not
      produce a request whose authority is `evil.example.zendesk.com`. It fails today. Name it.
      → `a_host_value_cannot_move_the_origin`, `crates/connector-pack/tests/configuration_value_guard.rs:105`
- [x] A path-position value containing `/`, `..`, `?`, `#`, `%2F` or a control character is refused or
      encoded — decide which per position and record the reason. A refusal must name the field, the
      operation and what is wrong with the value.
      → **refused**, not encoded: `Slot::Path` → `validate_path`, `crates/connector-pack/src/request.rs`.
      `Error::UnsafeConfig` names operation, variable, position and reason.
- [x] A query-position value goes through the existing `auth::query_encode` rather than a second
      encoder. → `Slot::Query` calls `crate::auth::query_encode`, which became `pub(crate)` for it.
- [x] **Whitespace-only values are covered.** `" "` currently survives the empty-string filter at
      `crates/connector-pack/src/config.rs:278` and reaches the wire as `?teamId=%20`. An
      all-whitespace configuration value is not a value.
      → the snapshot filter is now `!value.trim().is_empty()`, so whitespace reads as *absent* and
      produces `Error::MissingConfig` naming the field. `Slot::validate` refuses it a second time.
- [x] A raw newline cannot reach a **header** pin. No shipped provider declares one yet
      ([C-164](C-164-provider-algolia.md) will be the first), so this must be proved against a fixture
      rather than the catalogue — header injection is the one position here with a classic exploit.
      → `a_newline_cannot_reach_a_header_pin`, over a `Box::leak`ed catalogue entry carrying the
      header-pin shape `connector-flux` emits.
- [x] The validation runs at **substitution time** in `connector-pack`, so it binds every host and
      every `ConfigStore`, not only the loader's view of an `example`.
      → `Build::substitute`, the one substitution point. The best-effort egress-subject path
      (`Operation::substituted_host`) applies the same predicate so the gate and the wire agree.
- [x] `connector-spec`'s `Position::validate_value` is reused rather than reimplemented, or is
      deliberately replaced with the reason recorded. Two spellings of one rule is the defect this
      story is already an instance of.
      → **deliberately replaced, reason recorded** on `Slot` in `crates/connector-pack/src/request.rs`:
      `connector-pack` has no dependency edge to `connector-spec` (every `connector_spec` mention in
      the crate today is prose), and adding one is a manifest change with an architectural argument
      behind it. See `## Progress` — collapsing the two spellings needs its own story.

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

## Progress

**2026-07-31 — implemented on `impl/C-214`.** Whole gate green in the worktree; zero tests red.

### What each position answers, and why the three differ

`connector-pack/src/request.rs` gained `Slot` — where a configuration variable lands, **derived
from the operation's own emitted Flux** by `endpoint_slots`, in the same spirit as
`endpoint_variables`. The catalogue carries no `binds` target and no `Position` (that is C-87), so
waiting for the declaration to publish the position would have left the guard unbound for another
release. A unit test asserts every configuration variable in the shipped catalogue is placed, and
pins each placement by name.

| position | answer | reason |
|---|---|---|
| **host** | refuse unless the authority the *template* composes is a hostname | an allow-list of `[A-Za-z0-9._-]` with non-empty labels. No permitted character can delimit, so the resolved host is exactly the composed string — the fixed suffix the template declares therefore survives as a *consequence* rather than as a second rule. Fails closed on `@`, `:`, `/`, `%`, whitespace, control and non-ASCII alike, including the cases nobody enumerated. |
| **path segment** | refuse | the story's own reasoning: a `zone_id` with a `/` in it is an operator's mistake, and silently encoding it produces a 404 they cannot diagnose. |
| **query value** | refuse query *structure* (`& = ? # +`, whitespace, control), then `auth::query_encode` the rest | encoding an `&` would send `%26` where a separator was plainly meant. The remainder (`/`, `:`, `@`, `,`) has one meaning and is safe to encode. |
| **header value** | refuse | RFC 9110 §5.5. There is no encoding that makes a CR/LF safe in a field value. |
| **unplaced** | refuse unless *every* position accepts | unreachable for the current emitter, and it exists so a new emitted shape degrades into more refusal rather than none. |

The comparison is against the **template's** authority, not the finished URL's — that is the
load-bearing detail. `evil.example.zendesk.com` is a perfectly well-formed hostname, so reading the
authority off the built URL sees nothing wrong; the string the template composes carries the `@`.

### No shipped provider's behaviour changed

All 45 providers, 488 artifacts, `connector-cli diff` clean. The 13 configuration variables in the
catalogue are placed as expected (7 host, 5 path, 1 query), and legitimate values are untouched:
`teamId = "team_abc123"` is unreserved throughout, so `query_encode` is the identity over it.

### Follow-up worth its own story

**One rule, two spellings.** `connector-spec`'s `Position::validate_value`
(`crates/connector-spec/src/config.rs:269-333`) and `connector-pack`'s `Slot::validate` now say the
same thing in two crates. That is the defect this story is an instance of, and it was not closable
here: `connector-pack` has no dependency edge to `connector-spec`, and adding one is a manifest
change that couples the host-facing pack to a compiler crate `AGENTS.md` fences off — a decision
that deserves its own contract rather than a side effect of a security fix. The story's sanctioned
alternative ("deliberately replaced with the reason recorded") was taken; the correspondence is
documented on `Slot` with a file:line pointer for eye-diffing.

### Recorded, not settled

The story's open question — whether a consuming host's egress allow-list matches on prefix or path
rather than host — still lives outside this repository. What changed here is that a value the guard
refuses no longer reaches the subject either: `Operation::substituted_host` leaves the placeholder
verbatim, which no allow-list matches.

### Rework round 2 — three corrections from independent security review

The review passed the guard itself: 36 probes against the host rule (percent-encoded `%40`/`%2540`,
NUL, DEL, CRLF, `:8080`, `[::1]`, fullwidth `．` U+FF0E, ideographic `。` U+3002, one-dot-leader
U+2024, BOM, ZWSP, NBSP, NEL, `%2e%2e`) moved the origin in no case, and every new guard was
falsified individually. What it found was **three false or incomplete statements in my own
comments** — the artefacts a later reader trusts instead of re-measuring.

1. **"The correspondence is exact" was false.** Measured differentially over 33 values × 3
   positions, the two spellings diverge in exactly one place: `connector-spec` refuses `%` in a
   query value (`config.rs:303`, charset `&=?#+%`), this crate does not (`&=?#+`). The divergence
   is *principled* — the loader's own reason for refusing `%` is that nothing encodes a query value
   where it runs, which is true of an `example` and false here, because `query_encode` maps `%` to
   `%25`. The `Slot` doc now carries the charset table, the reason, and the observation that the
   asymmetry runs the **fail-safe** way: the loader is the stricter of the two, so a provider author
   cannot ship `example = "50%off"` for a query pin while a tenant may still supply one. Drift in
   the other direction is what would hurt, and
   `the_query_rule_admits_a_percent_because_it_encodes_one` is now where it would surface.
2. **`Slot::Unplaced` was documented as fail-closed and was not**, in the one position this story is
   about. It applied the path, query and header rules — none of which refuses `@` or `:`, because no
   such position cares about them — so `Slot::Unplaced.validate("acme.zendesk.com@evil.example")`
   returned `Ok`. Unreachable today (the reviewer confirmed every brace-carrying literal in the
   catalogue is either a full URL or a sole placeholder), so this was a defence-in-depth layer that
   did not defend rather than a hole. **The host rule is now in that arm**, and the test asserts both
   that `Unplaced` refuses the value *and* that the other three rules accept it — so the reason the
   host rule is needed there is executed rather than claimed.
3. **One of the three reasons for not reusing `Position::validate_value` did not survive checking.**
   I wrote that `AGENTS.md`'s dependency fence stood in the way. It does not: the fence is
   *directional*, forbidding compiler → host/network, and pack → spec is the opposite direction and
   unguarded (`dependency_fence` 4 passed, `publish_closure` 6 passed with the edge added in a
   throwaway copy). The recorded reason now leads with the stronger argument the reviewer surfaced:
   **`connector_spec::Position` has no `Host` variant**, so reuse could have covered at best three of
   these five slots, and `validate_authority` had to be written here regardless — the severe half of
   this story, the half predating C-187, was never reusable at all.
