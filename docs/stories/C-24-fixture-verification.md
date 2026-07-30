---
id: C-24
title: Verify generated connectors against recorded HTTP fixtures
pillar: Build
status: backlog
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-flux, connector-cli]
note: proves a connector *works*, not merely that it parses — without live credentials
---

# Verify generated connectors against recorded HTTP fixtures

## Goal
Close the gap between "the generated module parses" and "the generated module actually calls the API
correctly", without needing live credentials or network access in CI.

## Acceptance
- [ ] A recorded fixture format captures a request/response pair per operation: method, URL, headers
      (credentials redacted), body, status, response body.
- [ ] A test runs a generated op against its fixture and asserts the **request** it would send matches
      — correct method, URL with path params substituted, query params, headers, and body shape.
- [ ] Credential placement is asserted structurally (the right header name, the right placement)
      **without any real credential** in the fixture.
- [ ] Fixtures live under `specs/` or a sibling and are committed, so CI is hermetic and offline.
- [ ] At least one fixture per in-scope provider (zendesk, freshdesk, babelforce).
- [ ] A fixture that no longer matches its generated op fails CI, so a codegen regression is caught
      before anyone installs the connector.

## Progress
- (not started)

## Notes
- C-11 proves a module *parses and analyzes*. That is necessary and nowhere near sufficient: a module
  can be perfectly valid Flux and still build the wrong URL, drop a required query parameter, or put
  a credential in the wrong header. This story covers that gap.
- flux has a cassette concept (`../flux/crates/flux-flow/src/cassette.rs`) — worth reading before
  inventing a fixture format, since reusing its shape may let generated connectors be replayed by
  flux's own machinery rather than only by ours.
- Deliberately does **not** need the `$auth` seam: asserting *where* a credential would be placed is a
  structural check, so this can land while zendesk/freshdesk are still blocked on live calls.
