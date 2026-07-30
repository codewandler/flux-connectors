---
id: C-55
title: Let a provider declare a constant request header
pillar: Codegen
status: ready
priority: 2
design:
epic: connectors-v1
areas: [connector-spec, connector-flux]
note: GitHub's Accept header is undeclarable today; `const` on a header silently does nothing
---

# Let a provider declare a constant request header

## Goal
Give a connector a way to send the vendor-constant headers an API requires — `Accept`,
`X-GitHub-Api-Version`, `anthropic-version`, `User-Agent` — without turning them into arguments the
caller must pass and may overwrite.

## Acceptance
- [ ] A provider can declare a constant, non-credential request header, at provider level (applies to
      every operation) and at operation level. The emitted Flux carries it as a literal, not as a
      parameter.
- [ ] **`const` on a header param stops being a silent no-op.** `crates/connector-flux/src/op.rs:271`
      filters `constant(...)` on the `body` chain only, and `:487-492` emits every header param as a
      symbol — so a `const`-pinned header today emits as a required, caller-overridable argument with
      the constraint dropped. Either honour it or refuse it loudly, mirroring the `NestedBodyField`
      refusal. Silent is the one option this repo does not allow.
- [ ] `providers/github.toml` declares `Accept: application/vnd.github+json` and drops its
      `SCHEMA GAP:` note. A test asserts the header reaches the generated module.
- [ ] A constant header can never carry a credential: a test asserts the field rejects anything that
      resolves from an env var or the token store. Auth headers are the `$auth` seam's business (C-10),
      and this field must not become a second, ungated path to one.
- [ ] Behaviour for the five existing providers is unchanged — byte-identical artifacts unless a
      provider opts in.

## Progress
- Not started. Filed 2026-07-30 from C-52, where the gap was confirmed in the loader and the emitter
  rather than assumed.

## Notes
- Impact is version pinning, not function: GitHub defaults `Accept` to `application/vnd.github+json`
  when absent, so C-52's five operations are well-formed today. What is lost is the ability to pin the
  media type, so a vendor default change moves every operation's behaviour at once with nothing in the
  repo to hold it.
- Sibling of the `params.header` trap: the IR documents header params as caller-supplied, which is
  correct — this story adds the *other* kind rather than reinterpreting that one.
