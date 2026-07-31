---
id: C-217
title: Ship the Bitbucket connector
pillar: Spec
status: in-progress
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: every operation is scoped to a `{workspace}` PATH SEGMENT. C-187 just landed `binds = "path.<name>"` and nothing ships one — this is its first real consumer"
---

# Bitbucket — the first consumer of C-187's pinned path segment

## Goal

Ship a curated `bitbucket` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

[C-187](C-187-config-cannot-pin-a-request-component.md) landed `binds = "path.<name>"` so an
operator can pin a tenant scope at install time. **No shipped connector uses it.** A capability with
no consumer is a capability nobody has checked.

Bitbucket is the honest first consumer: every meaningful endpoint is under
`/2.0/repositories/{workspace}/...`, and a workspace is exactly the "once per installation, not once
per call" value C-187 exists for. Without the pin, every operation would carry a `workspace`
argument a model chooses each time — the Cloudflare problem, again.

## Acceptance

- [x] `providers/bitbucket.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [x] A `verify` operation that is an argument-free read and runs unattended.
- [x] `crates/connector-flux/tests/bitbucket_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [x] **Failing-first test:** the contract test must fail before `providers/bitbucket.toml` exists.
- [x] The scoped gate is green: `build --provider bitbucket`, `diff --provider bitbucket` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- Pin `workspace` as a path segment. That makes one installed connector address one workspace,
  which is the intended consequence — say so in the provider header the way `cloudflare.toml` does.
- **Read [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md) before writing the config
  help text.** A pinned path value is not validated at request time yet, so a workspace containing a
  slash produces a wrong URL rather than a refusal. Do not paper over it; if it bites your test,
  that is C-214's finding and worth reporting.
- Bitbucket Cloud and Bitbucket Server/Data Center are different APIs with different hosts. Ship
  Cloud and say that Server is out of scope, rather than leaving a reader to discover it.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.

## Progress

**2026-07-31 — implemented and scoped-gate green. Ready for coordinator integration.**

Built on `14ffbd4 wip(C-217)`, the loose tree of an agent killed by a session limit. That work was
unreviewed and ungated; it was audited against `main` after merging, and it held up. Two changes were
needed:

1. **`the_pinned_workspace_reaches_every_module_as_a_substitutable_placeholder` asserted a trailing
   slash** — `flux.contains("/repositories/{workspace}/")`. `bitbucket-repository-list`'s path is
   exactly `/repositories/{workspace}`, with the pin as its *last* segment, so the assertion failed on
   the one operation that most has to pass it: the argument-free `verify` this whole story exists to
   demonstrate. Narrowed to `"/repositories/{workspace}"` with a comment recording why the slash must
   not come back.
2. **`cargo fmt --all --check` was red** on the test file (`:469`, a `let` binding over the width
   limit). Formatted; the whole workspace is clean.

Nothing in `providers/bitbucket.toml` changed. It loads, and its claims about the loader — that
`binds = "path.workspace"` parses, derives `Level::Connection`, derives non-secret, and that
`Position::Path::validate_value` refuses a pasted URL — were each re-verified against current `main`
rather than taken on trust.

**Evidence.** Failing-first: with `providers/bitbucket.toml` moved aside, all 13 tests in
`bitbucket_connector.rs` fail with `cannot read …/providers/bitbucket.toml … — C-217 ships the
Bitbucket connector`. Restored: 13 passed. `build --provider bitbucket` writes 10 artifacts;
`diff --provider bitbucket` reports `10 artifacts up to date (1 provider checked)`. Workspace build,
clippy (`-D warnings`) and `fmt --check` are clean.

**The eight red whole-catalogue staleness tests are exactly the set `AGENTS.md` tabulates**, across
five binaries, and were left red deliberately — no full build was run. The ninth, coordinator-owned
`the_recorded_floor_is_the_measured_figure`, **passed**: this connector's 7 operations all carry
response shapes and fit inside the ratchet's slack alone. It may still go red on the *wave's*
accumulation, which is the coordinator's call, not this story's.

**Finding for a new story:** `Position::Path::validate_value` is never run against an operator's
actual value — that is [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md), reconfirmed
here rather than worked around. This connector is the sharpest instance of it yet, because a
workspace slug is a *fragment of a URL* and the natural mistake is pasting the URL it came from. Until
C-214 lands, the `help` text is the only thing preventing a wrong endpoint at the right vendor
carrying the operator's own token.
