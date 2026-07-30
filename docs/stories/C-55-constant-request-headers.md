---
id: C-55
title: Let a provider declare a constant request header
pillar: Codegen
status: done
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
- [x] A provider can declare a constant, non-credential request header, at provider level (applies to
      every operation) and at operation level. The emitted Flux carries it as a literal, not as a
      parameter.
- [x] **`const` on a header param stops being a silent no-op.** `crates/connector-flux/src/op.rs:271`
      filters `constant(...)` on the `body` chain only, and `:487-492` emits every header param as a
      symbol — so a `const`-pinned header today emits as a required, caller-overridable argument with
      the constraint dropped. Either honour it or refuse it loudly, mirroring the `NestedBodyField`
      refusal. Silent is the one option this repo does not allow.
- [ ] `providers/github.toml` declares `Accept: application/vnd.github+json` and drops its
      `SCHEMA GAP:` note. A test asserts the header reaches the generated module.
- [x] A constant header can never carry a credential: a test asserts the field rejects anything that
      resolves from an env var or the token store. Auth headers are the `$auth` seam's business (C-10),
      and this field must not become a second, ungated path to one.
- [x] Behaviour for the five existing providers is unchanged — byte-identical artifacts unless a
      provider opts in.

## Progress
- Filed 2026-07-30 from C-52, where the gap was confirmed in the loader and the emitter rather than
  assumed. Re-measured by C-107, which came back BLOCKED on it: Notion requires
  `Notion-Version: 2022-06-28` on every request, and the emitted op declared it as a caller-supplied
  argument, so every call would 400.
- **The mechanism is `const_headers`**, a table of literal `name = "value"` pairs at two levels: the
  file's top-level `[const_headers]`, and an operation's `[operations.params.const_headers]`. The
  loader distributes the provider's onto every operation (an operation's own entry replacing it,
  matched case-insensitively), so the IR always states the complete set an operation sends and no
  consumer resolves an inheritance. `ParamSet` is the home rather than `Operation`/`Connector`
  because a new field on either breaks struct literals in `crates/connector-cli/src/site.rs`, which
  another story owns.
- `const` on a `params.header` entry is now **refused** (`Error::ConstantHeaderParam`) rather than
  honoured, per this story's own note: `params.header` means caller-supplied, and reinterpreting one
  entry of it by schema keyword would make a single declaration mean two things.
- The credential rule is enforced at the loader, which is the only place that can see credential and
  env-var names: `Authorization`/`Proxy-Authorization`/`Cookie`, any header a declared `header`-scheme
  credential is injected into, a value beginning `Bearer `/`Basic `/`Token `, a value naming a declared
  credential or one of its env vars, and any `${…}`/`{{…}}`/`env:`/`$secret` spelling are all refused.
  So are a CR/LF in a value (header injection) and a `content-type` entry (the emitter derives it).
- **`providers/github.toml` was deliberately left alone.** Opting it in changes shipped artifacts and
  the whole-catalogue files a story implementor may not write, and the dispatch's gate requires
  `17 providers, 237 artifacts up to date; nothing written`. The mechanism is proved by fixtures
  instead — `crates/connector-flux/tests/constant_headers.rs`. GitHub's opt-in is a one-line
  `[const_headers]` table plus a regenerated artifact set, and is the natural first use.

## Notes
- Impact is version pinning, not function: GitHub defaults `Accept` to `application/vnd.github+json`
  when absent, so C-52's five operations are well-formed today. What is lost is the ability to pin the
  media type, so a vendor default change moves every operation's behaviour at once with nothing in the
  repo to hold it.
- Sibling of the `params.header` trap: the IR documents header params as caller-supplied, which is
  correct — this story adds the *other* kind rather than reinterpreting that one.
