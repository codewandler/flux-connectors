---
id: C-30
title: Encode scalar query values structurally and defer unmodelled arrays
pillar: Codegen
status: done
priority: 6
design: docs/designs/query-encoding.md
epic: connectors-v1
areas: [connector-flux]
note: **security** · Flux 0.54 has structured RFC 3986 query encoding; arrays stay withheld until their wire shape is declared
---

# Encode scalar query values structurally and defer unmodelled arrays

## Goal
Stop model- and operator-supplied query values from changing request structure. Emit scalar values
through Flux 0.54's structured `http.request(query: ...)` map, and withhold operations whose array
serialization is not declared rather than guessing a vendor convention.

## Acceptance
- [x] `connector-flux` emits every required, optional and operator-pinned scalar query parameter in
      the structured `query` object passed to `http.request`; the URL contains path data only.
- [x] The generated path accepts string, number and boolean values, preserves explicit `false` and
      `0`, omits absent optional values as `null`, and refuses array/object/unknown values with an
      `UnencodableQueryValue` naming C-30, the operation and the parameter.
- [x] `connector-pack` evaluates the structured query object and appends keys and scalar values with
      Flux's RFC 3986 semantics (`%20`, never `+`) exactly once, including configured query pins.
- [x] Provider overlays can defer one operation selected by a broad selector with a non-empty
      reason. Deferral is exact, fail-closed on absent or unmatched operations, and cannot be mixed
      with corrections to an operation that will not publish.
- [x] Asterisk ARI defers the 12 selected operations whose query parameters are arrays. Babelforce's
      18 string-or-array query parameters are narrowed explicitly to their documented scalar branch.
- [x] Generated connector, catalogue, lockfile and public-site artifacts are regenerated; the full
      Rust and web gates pass.

## Progress
- 2026-08-03 — Flux 0.54 is now the workspace baseline and implements the structured query contract
  specified by this story's design. The temporary "refuse strings" plan is superseded by the
  permanent structured path; only unmodelled collection serialization remains refused.
- 2026-08-03 — Failing-first emitter coverage produced the old `?query={query}` / `$sep` output, and
  the overlay fixture rejected the then-unknown `defer` key. The implementation now emits scalar
  query records, mirrors their exact wire URL in `connector-pack`, and fails closed on collection
  types or invalid deferrals.
- 2026-08-03 — Full integration measured 829 published operations and 1102 generated artifacts after
  withholding 12 Asterisk array-query operations. `connector-cli diff` reports `1102 artifacts up
  to date (55 providers checked)`; the full workspace build/tests/Clippy/format and web build/tests
  pass.

## Notes
- **This is a security finding, not a correctness one.** C-28 established that a query value of
  `x&per_page=1&admin=true` parses into three query parameters. `http.request` is a model-visible
  tool and connector query values are model-supplied, so this is parameter injection: it can widen a
  page cap, flip a pinned boolean, or collide with the `AuthScheme::Query` credential parameter the
  auth seam will add.
- **The obvious spot-check will mislead you.** `http.request` parses the assembled URL with
  `url::Url::parse` (`../flux/crates/flux-system/src/net.rs:126`), which already percent-encodes
  **spaces**. So `type:ticket status:new` — the canonical broken example — works today by accident.
  `&`, `#`, `+` and newline do not. Anyone verifying by trying a space will wrongly conclude the gap
  is closed.
- **The Flux prerequisite is now present.** Flux 0.54 reads `query`, accepts scalar values, omits
  null, rejects arrays/objects, uses RFC 3986 encoding, and refuses duplicate keys already embedded
  in `url`. C-30 now adopts that contract rather than its earlier temporary refusal.
- **Arrays remain a modelling decision.** APIs variously use repeated keys, comma-separated values,
  bracketed keys or JSON. The connector IR does not declare one of those, so emitting an array would
  replace a known absence with a plausible wrong request.
- Path parameters have the identical gap and are **not** covered here: `path_template` interpolates
  verbatim, so a string path parameter containing `/` or `?` escapes its segment. Harmless for the
  current inventory (all numeric ids) and not fixed by the structured `query` map.
