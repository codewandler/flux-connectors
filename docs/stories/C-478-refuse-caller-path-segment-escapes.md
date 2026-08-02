---
id: C-478
title: "Refuse caller path values that escape their segment"
pillar: Host
status: done
priority: 1
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [connector-pack]
note: "Messaging prerequisite — string ids may not turn one reviewed resource path into another"
---

# Refuse caller path values that escape their segment

## Goal

Apply the same segment-boundary rule already enforced for host-owned path pins to caller-supplied
path parameters before a generated request reaches egress.

## Acceptance

- [x] A failing-first pack test proves a caller path value containing `/`, `?`, `#`, `%`, `\\`,
      whitespace, a control character, `.` or `..` currently reshapes or escapes the reviewed URL.
- [x] The pack derives path-parameter placement from the emitted Flux declaration, including values
      used inside guarded branches, without relying on provider-specific names or catalogue IR.
- [x] Safe string and numeric path values remain byte-identical; query, header and body arguments are
      not accidentally subjected to the path rule.
- [x] An unsafe caller value returns a dedicated refusal naming the operation and parameter before
      the transport is invoked.
- [x] Request, rehearsal and full pack tests cover the positive path and every delimiter mutation.

## Progress

- 2026-08-02: re-read the pinned Messaging path parameters. `conversationId`,
  `userIdOrExternalId`, `integrationId`, and `webhookId` are unconstrained strings; only examples
  suggest their shape. `connector-spec::Position::Path` already refuses segment escapes for
  configuration pins, but `connector-pack` currently interpolates caller parameters without that
  check, contradicting C-460's promised refusal for external user ids.
- 2026-08-02: the failing-first probe showed `id = "a/b"` becoming two URL segments. The pack now
  derives caller path placement once from emitted Flux, recursively follows both guarded branches,
  and returns `UnsafePathParameter` before request construction for `/`, `?`, `#`, `%`, backslash,
  whitespace/control characters, `.` and `..`. Safe strings/numbers and query/header/body values are
  unchanged.
- 2026-08-02: the complete connector-pack suite passed: 84 library tests plus every integration and
  doc-test binary. The dedicated path suite passed 3/3, request 15/15, dry-run 8/8 and endpoint
  configuration 11/11; pack clippy, formatting and diff checks were green.
