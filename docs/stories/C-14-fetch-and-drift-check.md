---
id: C-14
title: Fetch specs and detect upstream drift
pillar: Build
status: backlog
design: docs/designs/connector-pipeline.md
epic: spec-front-end
areas: [connector-cli]
---

# Fetch specs and detect upstream drift

## Goal
Refresh the vendored spec cache deliberately, and make CI fail when a vendor's API has moved or a
generated artifact has gone stale — turning silent drift into a visible signal.

## Acceptance
- [ ] `flux-connectors fetch` downloads a provider's spec into `specs/<provider>/<version>.json`,
      honoring ETag / `If-None-Match` so an unchanged spec is not rewritten.
- [ ] `flux-connectors check` recomputes every hash in `connectors.lock` and **exits non-zero** on
      any mismatch — stale artifact, edited TOML, or changed spec — naming which provider and which
      input moved.
- [ ] `check` performs no network IO by default, so CI can run it offline; `check --upstream` opts
      into contacting vendors.
- [ ] `check` also verifies the flux-lang pin still matches what generated the artifacts, so a pin
      bump cannot silently invalidate output.
- [ ] Fetch failures are reported per provider without aborting the whole run.

## Progress
- (not started)

## Notes
- This is the story that answers action-proxy's third failure — hand-maintained config drifting from
  the real API forever. Drift is not preventable; it is *detectable*, and that is the whole design.
- Depends on `C-7`'s reproducible hashing.
