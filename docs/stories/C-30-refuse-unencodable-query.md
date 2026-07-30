---
id: C-30
title: Refuse query values the emitter cannot encode safely
pillar: Codegen
status: ready
priority: 6
design: docs/designs/query-encoding.md
epic: connectors-v1
areas: [connector-flux]
note: **security** · a model-supplied query value can inject request parameters today
---

# Refuse query values the emitter cannot encode safely

## Goal
Stop the emitter producing operations whose query values can be injected into, by refusing to emit
what it cannot encode — until flux gains a structured `query` map.

## Acceptance
- [ ] `connector-flux` gains an `UnencodableQueryValue` error following C-8's existing refusal
      pattern, naming this story and the operation and parameter involved.
- [ ] String-ish and `Any`-typed query parameters are refused; `Number` and `Boolean` are allowed.
      The narrow scope is deliberate — see the risk below.
- [ ] `zendesk-ticket-search` is refused rather than emitted, and the other six zendesk operations
      still emit. The connector is honestly 6/7 until flux lands the structured `query` map.
- [ ] A test asserts the refusal fires, and a golden pins that the six unaffected operations are
      unchanged.

## Progress
- (not started)

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
- **Emitting the fix early is worse than refusing.** Both halves verified by C-28: flux's analyzer
  *accepts* unknown call arguments (`analyze.rs:548-549`, "extra fields are not errors") and the
  runtime *ignores* them (`http.rs:137-160` reads only url/method/headers/body/timeout). So emitting
  a `query` map against an older flux silently drops every filter and returns 200 OK with the wrong
  result set.
- The permanent fix is a structured `query` map on `http.request`, drafted for flux in
  [query-encoding-flux-stories.md](../designs/query-encoding-flux-stories.md) (F-1). It must encode
  **RFC 3986**, not `append_pair`, which form-encodes space to `+`.
- **Known limit of this refusal:** it is type-based, so a free-form parameter mistyped as `Number` in
  a provider TOML is still emitted and still corrupts silently. The tighter alternative — refuse all
  query parameters — is recorded in the design and was not recommended.
- Path parameters have the identical gap and are **not** covered here: `path_template` interpolates
  verbatim, so a string path parameter containing `/` or `?` escapes its segment. Harmless for the
  current inventory (all numeric ids) and not fixed by the structured `query` map.
