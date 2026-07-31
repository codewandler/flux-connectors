---
id: C-237
title: "The host's explorer fires 30 requests to read three fields, has no search over 53 connectors, and throws away half of what the API returns"
pillar: Surfaces
status: ready
priority: 2
design: docs/designs/host-explorer.md
epic: host-explorer
areas: [host]
note: "phase 1 — everything here is already on the wire and needs no backend work. C-212 added operations[].requires and .callable specifically so the N+1 would not be needed, and the page still does it"
---

# The host's explorer is not yet a console

## Goal

Make the page usable at the size the catalogue actually is, in the single file that already exists,
with no build step and no new backend.

## What was measured

`crates/connectors-api/src/index.html`, 355 lines. Verified by grep returning **nothing** for
`filter`, `search`, `textarea`, `localStorage`, `history.pushState`, `DELETE`.

**1. An N+1 the backend already removed.** `show()` runs

```js
Promise.all(c.operations.map(o => api('GET', '/v1/operations/' + o.id)))
```

— up to ~30 requests per connector click, to read `tool`, `description` and `risk`.
[C-212](C-212-the-host-repeats-the-connected-conflation.md) added `operations[].requires` and
`.callable` **specifically so this would not be needed**; its own Progress note says the page "no
longer has to fetch 254 operation details to find out which of them it could run". It still does.

**2. No search, filter or sort** over 53 connectors and ~299 operations. The left rail is one flat
unsorted list.

**3. The page throws away what it is given.** `OperationView` returns `service`, `idempotency`,
`hosts` and `credentials`; the page renders `tool`, `description` and `risk`. `service` is the
addressing level [C-49](C-49-provider-services.md) established and the dimension `explorer-ux.md` says the
*public* explorer is also missing.

**4. The parameter editor is a single-line `<input value="{}">`.** Not a textarea, not schema-driven.

**5. The response is dumped verbatim.** `content` is one flat `HTTP {status}\n{headers}\n{body}`
string written into a `<pre>` with no status/headers/body split and no JSON formatting.

**6. `DELETE /v1/credentials/{provider}/{credential}` exists** at `crates/connectors-api/src/api.rs:403`
and is unreachable from the page. An operator can store a credential and cannot remove it.

## Acceptance

- [ ] **Failing-first test:** opening a connector issues **one** request per operation the operator
      actually expands, not one per operation in the connector. It fails today at ~30. Name it.
      (See [C-239](C-239-a-test-harness-for-the-host-page.md) — if no harness exists yet, state how
      this was proved instead of asserting it was.)
- [ ] Search and filter over the connector list, including by wiring state. *"Which of these still
      need setup"* is the operator's real question and `wiring` already answers it.
- [ ] Operations are grouped by `service`, and `idempotency` and `hosts` are rendered rather than
      discarded.
- [ ] A parameter editor that can hold a real body, with invalid JSON refused before sending rather
      than by the vendor.
- [ ] The response is legible: status, headers and body distinguished, JSON bodies formatted. The
      redactor's output must pass through unchanged — it is what stops a vendor echoing a token onto
      this surface.
- [ ] A credential can be removed through the page.
- [ ] **Layout hygiene, borrowed rather than rediscovered.** `min-width: 0` on flex and grid
      children, `flex-wrap: wrap`, `overflow-wrap: anywhere`. `OperationRow.vue`'s comment records
      what their absence cost the site: a path like `/v0/{baseId}/{tableIdOrName}/{recordId}` has no
      break opportunity, so every row forced its track wider than the viewport and the page scrolled
      sideways by ~193px.
- [ ] Everything in `docs/designs/host-explorer.md` §"Constraints any implementation must hold" still
      holds — in particular `textContent` never `innerHTML`, POST-not-link for auth state changes,
      the dev button only under `status.dev`, and the `wiring` tokens character-identical to C-206's.
- [ ] `cargo test --workspace --no-fail-fast` green, with
      `tests/host.rs::a_stored_credential_reaches_no_surface` and
      `without_a_google_registration_the_host_still_starts_and_explains_itself` both still passing.

## Notes

- **Scope: this file only.** No build step, no npm, no framework, no external assets. That is
  [C-238](C-238-the-host-mounts-the-explorer-components.md)'s job and doing it here would make both
  harder.
- Nothing here needs a backend change. If something does, that is a finding worth its own story
  rather than a widening of this one.
- Two open stories will land in this file — [C-225](C-225-a-config-field-cannot-declare-a-closed-set-of-values.md)
  and [C-226](C-226-one-credential-cannot-be-shared-by-two-connectors.md). Leave room; do not
  implement them here.
- The configuration binder is a raw four-input row that requires the operator to know the field name,
  seeded only from the first `{var}` in `base_url`. C-225 is the story that makes it a choice — worth
  reading before touching it.
