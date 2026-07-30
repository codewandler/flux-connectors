---
id: C-18
title: Vendor the babelforce spec and curate the provider operation inventory
pillar: Spec
status: done
priority:
design: docs/designs/provider-operation-inventory.md
epic: connectors-v1
areas: [connector-spec, providers]
note: inventory delivered · spec vendoring split to C-25 (credential literals)
---

# Vendor the babelforce spec and curate the provider operation inventory

## Goal
Produce the raw material `providers/{zendesk,freshdesk,babelforce}.toml` will be written from (C-17):
a vendored, hermetic spec cache and a curated per-provider operation inventory with a precisely
recorded auth model. Research and curation only — no Rust, no `providers/*.toml`.

## Acceptance
- [ ] ~~The babelforce OpenAPI document is vendored under `specs/babelforce/`~~ — **moved to C-25.**
      The implementor produced a byte-identical copy, but it carries live-shaped credential literals
      and was deliberately **not** merged. See the coordinator note in Progress.
- [x] The upstream path, `info.version` and a sha256 of the upstream file are recorded in the
      inventory doc (the recorded sha256 is of upstream, which C-25 will need).
- [x] `docs/designs/provider-operation-inventory.md` records, per provider, the **selected**
      operations with method, path, parameters (name / in `path|query|header|body` / type /
      required) and a one-line description.
- [x] Zendesk's set matches what `../flux/plugins/zendesk/src/main.rs` exposes (ticket
      search/show/update, comment list/add, tag add, test).
- [x] Freshdesk's set is a **curated** ticket-centric selection, not the whole collection.
- [x] Babelforce's 163 operations are reduced to a usable handful, with inclusions **and**
      exclusions justified.
- [x] Auth is recorded per provider in the `flux_plugin_protocol::AuthScheme` vocabulary
      (`bearer` / `basic` / `header{name}` / `query{name}`) with the env var names holding the secret
      and, for Basic, the non-secret user half.
- [x] The requirement-set shape (AND / OR / none) is recorded per operation.
- [x] Babelforce's auth is recorded as **SSO-issued Bearer**, with the **JWT intent** captured so
      C-10's manifest schema is designed to accept it.
- [x] The deprecated `accessId` / `accessToken` (`X-Auth-Access-Id` / `X-Auth-Access-Token`) apiKey
      pair is recorded as **excluded, with the reason**, explicitly enough that a later reader does
      not "fix" its absence by re-adding it.
- [x] Zendesk and Freshdesk are stated plainly to be hand-derived, with where a real spec would come
      from later.
- [x] **No credential value** appears anywhere — env var names only.

## Progress
- **Both deliverables are on `impl/C-18`, but merge is gated on one unresolved safety question.**
- 🚫 **BLOCKER for the coordinator — `provider-operation-inventory.md` §1.3.** The upstream babelforce
  spec embeds a response `example` containing a live-shaped credential set for a `Testers Inc.`
  account: an `accessId`/`accessToken` pair and a stream token (4 token-shaped hex literals in total),
  plus a named `@babelforce.com` address. This repository is **public**
  (`repository = "https://github.com/codewandler/flux-connectors"`, `Cargo.toml:12-13`) and the
  upstream repo is private, so vendoring publishes them into a permanent history. The story requires
  a **byte-identical** copy with a recorded sha256, which is in direct tension with scrubbing them.
  **Confirm-and-rotate with the babelforce API owners, then choose a vendoring policy** (§1.3
  recommends a declared, reproducible scrub with both hashes recorded) before this branch lands on a
  published branch. Not a decision for the implementing agent — the file is **staged, not cleared**.
  Nothing else in the deliverables depends on the outcome.
- Vendored `specs/babelforce/manager-0.7.0.openapi.json` — byte-identical to upstream, sha256
  `6a79679409787c4ab1716936bca987226aacdc28eeff19039c0ea5ea34285421`, OpenAPI 3.0.3, `info.version`
  0.7.0, 98 paths / 163 operations. The upstream repo was not modified.
- Wrote `docs/designs/provider-operation-inventory.md`. Selection: zendesk **7 of 7**, freshdesk
  **9 of 16**, babelforce **9 of 163** — 25 operations of 186 available.
- Every parameter table and auth claim carries a `path:line` citation, and every cited line number
  was re-read and corrected against the source after drafting.
- **Three findings in §6 change other stories** and need the coordinator's attention before C-17
  and C-10 are picked up:
  1. **§6.1** — babelforce's deprecated header pair was C-10's motivating AND-set example and C-17's
     "executable today with no `$auth` seam" provider. Bearer needs the `Bearer ` prefix, so
     babelforce is now *also* blocked on the seam. **No provider is executable against flux as it
     stands today**, which is a milestone-1 sequencing change. C-10's AND-set test needs a different
     fixture; C-17's first Acceptance item is now false as written.
  2. **§6.2** — Freshdesk is `base64(<api_key>:X)`: the secret sits in the **username** position.
     `AuthMethod::basic` composes `base64(<user_env>:<env>)` with `user_env` documented as
     *non-secret config*, so expressing Freshdesk today would push the API key outside secret gating
     and outside redactor registration. That is a security regression and a C-16 decision, not
     something to work around in a provider TOML.
  3. **§6.5** — babelforce's `listReportingCalls` declares every filter twice (flat and
     `filters.`-prefixed, 40 params where ~20 are meant), several parameter names contain dots, and
     `servers[0]` is **staging**. Ingest (C-4) must not take `servers[0]` positionally.
- Deliberately **not** done: no `providers/*.toml` (C-3's schema does not exist yet), no Rust, no
  network fetch.

## Coordinator note — why the vendored spec was not merged

The implementor's finding was verified and upheld. `specs/babelforce/manager-0.7.0.openapi.json`
embeds a response example holding a 32-hex `accessToken`, a 64-hex stream token, an account UUID and
a real `@babelforce.com` address. This repo is intended to be public, git history is permanent, and a
later scrub would not undo it.

The implementor committed the spec in `54ef636` and the inventory in `dbaaed2` — cleanly separated.
Only `dbaaed2` was taken (cherry-picked), so **the credential-bearing blob never entered `main`'s
history**. `impl/C-18` retains it and must never be merged or pushed as it stands; nothing has been
pushed anywhere (the repo has no remote).

Vendoring continues as **C-25**, which is blocked on a human decision. Escalating rather than
deciding was the right call.

## Notes
- **This story file did not exist when the work was dispatched** — it was created here from
  `_TEMPLATE.md` using the Goal and Acceptance given in the dispatch. `status`, `priority` and the
  board row are the coordinator's to confirm.
- Sources, all read-only and unmodified:
  - zendesk — `../flux/plugins/zendesk/src/main.rs` (687 lines)
  - freshdesk — `~/babelforce/projects/integrations/action-proxy/dist/collections/freshdesk/`
    (`freshdesk.yml`, 649 lines · `template.yml`)
  - babelforce — `~/babelforce/projects/babelforce-api/babelforce-api/openapi/manager.openapi.json`
- action-proxy was mined for **endpoint facts only**. Its defects are catalogued in §6.3 and §6.4 as
  evidence for the pipeline argument, not copied forward.
