---
id: C-415
title: "Vendor the five babelforce manager specs, scrubbed and provenanced"
pillar: Build
status: in-progress
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [specs, connector-cli]
note: "the blocker providers/babelforce.toml:7-15 has recorded since C-17, resolved: the spec bytes are publishable, the GitLab fetch configuration is not — and the Testers Inc. accessId/accessToken examples come out"
---

# Vendor the five babelforce manager specs, scrubbed and provenanced

## Goal
Put the five babelforce OpenAPI documents under `specs/babelforce/` with their provenance recorded and
nothing in them that a public repository must not carry — so `[spec]` has something to point at.

## Acceptance
- [ ] Five documents land under `specs/babelforce/`: `manager`, `user`, `auth`, `task-automation`,
      `task-schedule`, taken from `~/babelforce/projects/manager/manager-sdk/specs/` — i.e. **after**
      that repo's `pull.sh` has normalized `servers:` to the public production host and applied its
      four generator-compatibility fixes.
- [ ] **The credential-shaped examples are gone.** `accessId: 036fea61-…` and
      `accessToken: 829b0c86…` for the `Testers Inc.` account appear at `user.openapi.yaml:293` and
      `manager.openapi.yaml:25935` in the source. A test greps the vendored bytes for a 32-hex literal
      and a bare UUID under an `accessId`/`accessToken` key and fails on either.
- [ ] **No internal marker reaches the repo.** A test applies
      `manager-sdk/scripts/leak-markers.regex` (`gitlab\.stack`, `sbf/services`, `latest\.dev`, …) to
      everything added here and fails on a hit. `sources.json` and `pull.sh` are **not** copied in —
      they hold the GitLab host and project ids and are the thing that must stay internal.
- [ ] The scrub is a **script in this repo**, not a manual edit — `scripts/`, runnable, so re-vendoring
      is reproducible and reviewable as a diff.
- [ ] Provenance is recorded per document: `sha256` of the vendored bytes and a version. `info.version`
      is `0.0.0-dev` on three of the five, so the file name carries the pull date and the sha256 is the
      real identity. `source_url` is omitted rather than pointing at an internal host — `SpecSource`
      already makes it `Option`.
- [ ] `AGENTS.md`'s vendoring policy states the split in one paragraph: pulled bytes are vendored here,
      pull configuration is not.

## Progress
- (not started)

## Notes
- **Do not "fix" the deprecated header pair back in.** The examples being scrubbed belong to
  `X-Auth-Access-Id`/`X-Auth-Access-Token`, which `providers/babelforce.toml:75-96` refuses to model
  deliberately. Ingest must keep *seeing* the scheme so drift-check reports on it; only the example
  **values** come out.
- `leak-markers.regex` explicitly excludes `X-Auth-Access-*` because the header *names* are public API.
  The example values are a different question and are what this story removes.
- Confirm-and-rotate with the babelforce API owners is worth asking for separately. It is **not** a
  gate on this story — the values leave this repo either way.
- No Rust changes. This story is data plus a script plus a policy paragraph, which is what makes it
  safe to run beside C-4 and C-413.
