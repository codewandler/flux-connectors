---
id: C-528
title: "A credential declares whose authority it carries"
pillar: Spec
status: done
priority: 0
design: docs/designs/unified-auth.md
epic: connector-config
areas: [connector-spec, catalog, connector-cli, connector-pack, tests]
note: "the on-behalf-of axis — Slack's one OAuth grant returns a bot token and a user token that are placed identically, acquired identically, and differ only in who they can act as"
---

# A credential declares whose authority it carries

## Goal

Let a connector state whether a credential acts as the **integration** or as the **person who
granted it**, so a host can bound its reach and decide where it may be stored.

Slack is the case that forces it. One OAuth v2 grant returns two tokens in one response —
`access_token` is the workspace bot (`xoxb-`) and `authed_user.access_token` is the signed-in person
(`xoxp-`). They are placed identically (`Authorization: Bearer …`), acquired by the same grant, and
differ in nothing any existing axis can express, while differing entirely in who they act as and how
much they reach. `providers/slack.toml` had the fact in prose — *"a bot posts as the app, so a write
is attributable to the integration rather than impersonating the operator"* — and no field to put it
in.

Three consequences ride on the distinction, which is why it is a declaration rather than a host
convention:

- **Blast radius.** An app-subject token carries the integration's grant across the whole workspace;
  a user-subject token is bounded by one person's permissions.
- **Where it is stored.** An app credential is provisioned once per tenant; a user credential once
  per person, and keeping one at a tenant-wide address would let one member act as another.
- **Whether delegation is possible at all.** Only a user-subject credential can answer "act on behalf
  of the signed-in user".

## Acceptance

- [x] `connector_spec::Subject` — `unstated` | `app` | `user` — with `AuthMethod::subject`.
- [x] `catalog::Subject` mirrors it and `catalog::Credential` publishes it, because the catalogue is
      what Exchange and autodev link. Same argument as [C-525](C-525-publish-oauth2-acquisition-in-the-catalogue.md):
      a declaration that stops at the IR is one no host can act on.
- [x] **The default is `unstated`, and it means "nobody reviewed this" rather than `app`.** A
      consumer needing the distinction refuses on it; assuming `app` over-grants and assuming `user`
      silently fails.
- [x] The unreviewed default is skipped when serialized, so **no already-published manifest moved**.
      Only `connectors/slack.connector.toml` changed, by the two lines Slack now declares.
- [x] The catalogue publishes `Unstated` explicitly rather than omitting it — a host must be able to
      see the difference between "this is an app token" and "nobody checked".
- [x] `providers/slack.toml` declares both credentials `app`, each with its own reason recorded.
- [x] The published `provider-toml.schema.json` documents the field; the loader's unknown-key golden
      snapshot is regenerated.
- [x] Failing-first tests cover the unstated default, both real answers side by side on one
      connector, the skip-on-default serialization, and the catalogue emission of all three states.
- [x] `diff` reports 1108 artifacts up to date; `fmt`, `clippy -D warnings` and the connector-spec,
      -pack, -catalog, -cli and -flux suites are green.

## Progress

- 2026-08-11: Implemented. Slack is the only connector reviewed so far; the other 54 carry
  `unstated` truthfully.

## Notes

**Why not follow C-516 and require every connector to state it?** That is the stronger design and it
is what direction got. It is not available here yet: 55 connectors ship credentials today and none
has been reviewed, some genuinely ambiguously. `providers/github.toml`'s single `github.token` is
documented as covering GitHub App installation tokens (app-subject) *and* personal access tokens
(user-subject) — one declaration standing for two opposite answers, which a forced choice would
resolve by guess. `unstated` records the real state of the tree; a default of `app` would have been
the marking AGENTS.md warns about, reading as a safety decision while recording only that the
question was never asked.

**This is a breaking change to a published crate.** `catalog::Credential` gains a field and is
deliberately not `#[non_exhaustive]`, so a consumer constructing one in a test stops compiling — the
break `connector-pack`'s own fixtures took here. Every generated table was rewritten; no `.flux`
byte and no manifest but Slack's moved.

Splitting the *user* subject out of GitHub's `github.token`, and the per-principal credential address
that a user-subject credential needs, are both follow-ups. The address may already work: the
credential path's `@instances/<uuid>` segment exists so one tenant can hold several connections to
one vendor, and "Alice's Slack" is another instance with the host mapping principal → uuid.
