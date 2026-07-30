---
id: C-28
title: Resolve percent-encoding for query values
pillar: Codegen
status: done
priority:
design: docs/designs/query-encoding.md
epic: connectors-v1
areas: [connector-flux, flux-bridge]
note: blocks zendesk ticket search · flux has no urlencode op at all
---

# Resolve percent-encoding for query values

## Goal
Decide and specify how a generated op percent-encodes query values, so a search expression containing
a space or `&` produces a correct request instead of a corrupted one.

## Acceptance
- [x] The investigation is written up: can this be solved connector-side at all, or does flux need a
      new op? Answer with evidence from flux's registered-op catalog, not from assumption.
      → [query-encoding.md](../designs/query-encoding.md) §2 (the gap, three independent checks) and
      §3.1 (connector-side: **no**, each candidate refuted).
- [x] If flux needs an op, a paste-ready story draft lands in
      [auth-seam-flux-stories.md](../designs/auth-seam-flux-stories.md)'s sibling handoff style,
      naming its failing-first test.
      → [query-encoding-flux-stories.md](../designs/query-encoding-flux-stories.md), a **sibling**
      file (rationale in its header). F-1/C-277 names
      `query_map_values_are_rfc3986_percent_encoded_not_interpolated`; F-2/C-278 names
      `composite_op_with_an_optional_param_runs_when_that_arg_is_omitted`.
- [x] The connector-side behaviour is specified: what the emitter does today, what it will do, and
      what it must **refuse** to emit rather than emit incorrectly.
      → [query-encoding.md](../designs/query-encoding.md) §4, including the
      `UnencodableQueryValue` variant and why the refusal is narrow (string-ish params only).
- [x] `zendesk.ticket.search` is shown either working or explicitly blocked with the reason.
      → [query-encoding.md](../designs/query-encoding.md) §5: **blocked**. The other six zendesk
      operations are unaffected — the connector is 6/7 today.

## Progress
- **Done — investigation and specification complete.** Two docs:
  [query-encoding.md](../designs/query-encoding.md) (the design record) and
  [query-encoding-flux-stories.md](../designs/query-encoding-flux-stories.md) (two paste-ready flux
  stories, provisionally C-277 and C-278).
- **The gap is real; C-8 missed nothing.** No registered op encodes (ops-reference table, a
  `ToolSpec` name census across `flux-tools`/`flux-web`/`flux-flow`, and a sweep of all of `crates/`
  for an op name matching `encode|escape|url|quote|uri` — all empty), and the `expr` builtin
  whitelist is a closed list of 21 names with no encoder
  (`../flux/crates/flux-lang/src/expr.rs:804-828`).
- **It is worse than "spaces break", and also narrower.** `http.request` parses the assembled URL
  with `url::Url::parse` (`../flux/crates/flux-system/src/net.rs:126`), which rescues **spaces** —
  so `type:ticket status:new`, the canonical example in the Notes below, works today by accident.
  `&`, `#`, `+` and newline do not. A value containing `&per_page=1&admin=true` **injects query
  parameters**, which makes this a safety finding, not only a correctness one.
- **Recommendation: a structured `query` map on `http.request`**, not a pure `urlencode` op. The op
  form is a smaller diff but is opt-in, so it leaves the injection vector open for any caller who
  forgets — including a model calling `http.request` directly. Both are weighed in §3.2.
- **Encoding must be RFC 3986, not `append_pair`.** `url::Url::query_pairs_mut().append_pair` form-
  encodes (space → `+`), which is exactly what the zendesk plugin was written to avoid. Flagged as a
  correction to C-271's Notes in `auth-seam-flux-stories.md:435-436` — a human pasting C-271 should
  read C-277 first.
- **Next, for whoever picks this up:** the `UnencodableQueryValue` refusal in `connector-flux` is
  specified but **not implemented** — it needs its own story (this one is investigation-only, and
  `crates/` was owned by other in-flight work). C-17 should leave `zendesk.ticket.search` out of the
  zendesk provider meanwhile.
- **Second question answered** (§6): the no-optional-composite-param gap is confirmed and is *larger*
  than the note below implies — `flux_lang::ast::Param` has no optionality field at all
  (`../flux/crates/flux-lang/src/ast.rs:323-326`), so it is an AST/parser/formatter change, not a
  `registry.rs` two-liner. Judged a real flux gap worth a story (F-2) but **not** on this one's
  critical path: the emitter's null-plus-`when` workaround is adequate for the wire, and what it
  cannot fix is the advertised signature.
- **Unverifiable citation flagged:** `../flux/plugins/zendesk/src/main.rs` does not exist at flux
  `v0.38.0` (absent from disk and from `git ls-files`), so
  [provider-operation-inventory.md](../designs/provider-operation-inventory.md) §3.3.5's citation
  cannot be re-checked. Nothing in this investigation depends on it. Affects C-17/C-18.

## Notes
- **Found by C-8, and it is not a small gap.** Query values are not percent-encoded, and flux has
  **no op that would do it** — the whole registered catalog was checked
  (`../flux/crates/flux-flow/docs/ops-reference.md`). A value containing a space, `&`, `#` or `=`
  corrupts the query string.
- Zendesk search expressions are exactly that shape (`type:ticket status:new`), and flux's own
  zendesk plugin percent-encodes them strictly for this reason
  (`provider-operation-inventory.md` §3.3.5). **So this blocks `zendesk.ticket.search`**, one of the
  seven operations the zendesk connector must cover to replace the plugin.
- C-8 deliberately did **not** half-fix it with `expr`'s `replace` for spaces only: that would look
  correct and be wrong, which is worse than an honest gap.
- Related flux-side gap, worth folding into the same investigation: flux has **no optional
  composite-op parameter** (`registry.rs:183-184` puts every param in `required_params`), so a model
  calling a six-filter op must pass six arguments to filter on one. Separate problem, same
  "flux needs a small change" bucket.
