---
id: C-475
title: "Reuse a Basic username as a host-owned request pin"
pillar: Spec
status: done
priority: 13
design: docs/designs/popular-provider-spec-coverage.md
epic: popular-provider-spec-coverage
areas: [connector-spec, connector-flux, connector-pack]
note: "Twilio prerequisite — one Account SID must drive both Basic auth and selected operation paths"
---

# Reuse a Basic username as a host-owned request pin

## Goal

Let one non-secret Basic username configuration field also fill an explicitly omitted OpenAPI path
parameter, without creating a second prompt, changing existing pin renderings, or resolving the value
through the wrong host-side configuration address.

## Acceptance

- [x] Failing-first tests prove a `username.<credential>` field with `also_binds = ["path.<name>"]`
      is currently refused and, if merely emitted, would be looked up as an endpoint value.
- [x] A username-headed request pin carries an unambiguous qualified placeholder and
      `connector-pack` resolves it through `Field::Username`; existing endpoint- and request-headed
      pins remain byte-identical.
- [x] Shared-slot validation compares the full `(kind, target)` address: duplicate fields still
      refuse, while the ordinary username/secret halves of one Basic credential do not collide.
- [x] A selected OpenAPI path parameter may be explicitly omitted only when an exact path pin in the
      same service owns its placeholder; every other path omission remains refused.
- [x] Request composition uses one Account SID for the Basic user half and the URL path, refuses a
      missing value before egress, and retains the position-specific unsafe-value checks.
- [x] Focused specification, Flux-emission and pack request tests are green before C-473 resumes.

## Progress

- 2026-08-02: `cargo test -p codewandler-connector-spec --test twilio_spec_selection -- --nocapture`
  exited 101. The loader reported both the selected operations' caller-owned `AccountSid` path
  parameters and a false collision between `username.twilio.basic_auth` and
  `credential.twilio.basic_auth`; tracing the emitted placeholder into `connector-pack` showed a
  third failure, because every surviving brace was resolved as `Field::Endpoint`.
- 2026-08-02: username-headed pins now emit the reserved qualified placeholder
  `username.<credential>`; `connector-pack` maps it back to `Field::Username`, snapshots the Basic
  user once, and applies the existing path-safety check at substitution. An imported path parameter
  may leave only through an explicit `omit.path` backed by an exact same-service pin.
- 2026-08-02: the focused coordinator rerun passed 57 connector-spec tests, 16 connector-flux tests,
  and the pack's exact username-pin rehearsal. The latter composed the configured Account SID into
  the path and proved both missing and path-reshaping values refuse before egress.
- 2026-08-02: the catalogue-wide request-composition gate exposed one remaining test-contract gap:
  its provider declaration reader still classified a `username.*` head bind as credential-only,
  although C-475 deliberately makes that qualified placeholder a Configuration field. It therefore
  reported `twilio-recording-list` as undeclared before exercising the request. The story remains
  open until that gate recognizes the declared qualified slot and passes.
- 2026-08-02: the catalogue declaration reader now preserves the full
  `username.twilio.basic_auth` placeholder and populates it through `MemoryConfig::with_username`;
  `credential.twilio.basic_auth` remains excluded from non-secret configuration. The focused
  assertion and all 15 request tests, including the whole-catalogue composition gate, pass.
