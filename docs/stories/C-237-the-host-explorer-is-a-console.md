---
id: C-237
title: "The host's explorer fires 30 requests to read three fields, has no search over 53 connectors, and throws away half of what the API returns"
pillar: Surfaces
status: in-progress
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

- [x] **Failing-first test:** opening a connector issues **one** request per operation the operator
      actually expands, not one per operation in the connector. It fails today at ~30. Name it.
      (See [C-239](C-239-a-test-harness-for-the-host-page.md) — if no harness exists yet, state how
      this was proved instead of asserting it was.)
      → `ui/test/host-page.test.mjs::opening a connector fetches no operation detail, and expanding
      one fetches exactly that one`, red at the merge base with the 30 URLs quoted, and
      `tests/catalogue_response.rs::the_connector_list_carries_every_field_its_rail_renders` on the
      server side of it.
- [x] Search and filter over the connector list, including by wiring state. *"Which of these still
      need setup"* is the operator's real question and `wiring` already answers it.
      → `src/index.html:227-272` (`#search` over id, vendor, description and every operation's id,
      tool, service and description) and `BANDS` at `:210-215`, keyed on the host's own `wiring`
      tokens. Tests 8 and 9 of the Node suite.
- [x] Operations are grouped by `service`, and `idempotency` and `hosts` are rendered rather than
      discarded. → `src/index.html:453-484`; Node test 7.
- [x] A parameter editor that can hold a real body, with invalid JSON refused before sending rather
      than by the vendor. **A schema-driven form needs no codegen**: `connector_pack::project(&entry)`
      is public (`crates/connector-pack/src/spec.rs:110`) and yields a real `input_schema`, so
      `OperationView` can carry one for ~5 lines. Every declared parameter is required by
      construction, so the form is flat — one control per property, no optionality logic. Keep the
      raw-JSON escape hatch.
      → `src/index.html:574-625`. `input_schema` reaches `OperationView` from
      `connector_pack::project` (`src/api.rs:528`); Node test 12 types `{ "ticket_id": }` and
      asserts nothing reached `/execute`.
- [x] The response is legible: status, headers and body distinguished, JSON bodies formatted. The
      redactor's output must pass through unchanged — it is what stops a vendor echoing a token onto
      this surface. → `renderResponse`, `src/index.html:514-536`. Node test 15 covers both halves:
      the canonical document is split, and a response that is not one is shown whole with
      `[REDACTED]` intact.
- [x] A credential can be removed through the page. → `src/index.html:358-370`; Node test 12 asserts
      the `DELETE /v1/credentials/fixture/fixture.api_key` that goes out.
- [x] **Optional but high value: a dry-run preview.** `Operation::project(...).dry_run(params)`
      (`crates/connector-pack/src/tool.rs:271`) renders the exact request without sending it and
      without touching the secret store — C-145's seam. It answers *"why will this not work"*
      precisely: `MissingConfig` names the field and its service, `MissingCredential` names the
      address. Verify that it refuses usefully for an unbound configuration before committing to the
      panel's copy; that has not been run.
      → It has been run now, and it refuses as promised:
      `tests/dry_run.rs::an_unbound_configuration_field_is_refused_by_name` holds a 400 for
      `zendesk-ticket-show` to naming `subdomain`, its service and the operation.
      `POST /v1/operations/{id}/dry-run` is `src/lib.rs:107`, `src/api.rs:722`, `src/exec.rs:121`.
- [x] **Layout hygiene, borrowed rather than rediscovered.** `min-width: 0` on flex and grid
      children, `flex-wrap: wrap`, `overflow-wrap: anywhere`. `OperationRow.vue`'s comment records
      what their absence cost the site: a path like `/v0/{baseId}/{tableIdOrName}/{recordId}` has no
      break opportunity, so every row forced its track wider than the viewport and the page scrolled
      sideways by ~193px.
      → `src/index.html:115-116` plus `flex-wrap: wrap` on six row rules, and — added on resume —
      `ui/test/host-page.test.mjs::a row with nowhere to break shrinks rather than widening the
      page`, which reads `getComputedStyle` on rows the page really drew rather than grepping the
      stylesheet for three strings.
- [x] Everything in `docs/designs/host-explorer.md` §"Constraints any implementation must hold" still
      holds — in particular `textContent` never `innerHTML`, POST-not-link for auth state changes,
      the dev button only under `status.dev`, and the `wiring` tokens character-identical to C-206's.
      → Node tests 1–5 (all four constraints) and `tests/wiring_vocabulary.rs` (2), all green. These
      are the ones the parking note recorded as **unproven**: the suite could not run in the parked
      worktree because it had no `node_modules`.
- [x] `cargo test --workspace --no-fail-fast` green, with
      `tests/host.rs::a_stored_credential_reaches_no_surface` and
      `without_a_google_registration_the_host_still_starts_and_explains_itself` both still passing.
      → 170 targets, **1568 passed, 0 failed**, and both named tests run and pass.

## Notes

- **Scope: this file only.** No build step, no npm, no framework, no external assets. That is
  [C-238](C-238-the-host-mounts-the-explorer-components.md)'s job and doing it here would make both
  harder.
- **The N+1 fix is a small backend change, not a client one.** Add `service`, `description`, `risk`,
  `idempotency` and `tool` to `OperationWiring` (`crates/connectors-api/src/api.rs:137`), filled in
  `view_of` from the entry already in hand. ~15 lines, and it turns ~30 requests per click into zero.
  Watch the response size: 299 operations adds roughly 55 KB uncompressed. Make "the list response
  stays under a stated size" an acceptance line rather than splitting the view type.
- **The search and sort logic here is a knowing duplicate.** `web/data/catalog.mts` already has
  `encodeView`/`decodeView`/`narrowView`/`sortOperations`/`facet`/`serviceFacet` as pure functions.
  This story hand-rolls the subset it needs, in one file, deliberately —
  [C-238](C-238-the-host-mounts-the-explorer-components.md) deletes the duplicate. Say so in the code
  so nobody defends it later.
- Two open stories will land in this file — [C-225](C-225-a-config-field-cannot-declare-a-closed-set-of-values.md)
  and [C-226](C-226-one-credential-cannot-be-shared-by-two-connectors.md). Leave room; do not
  implement them here.
- The configuration binder is a raw four-input row that requires the operator to know the field name,
  seeded only from the first `{var}` in `base_url`. C-225 is the story that makes it a choice — worth
  reading before touching it.

## Progress
- **2026-08-01 — parked mid-flight, work preserved on `impl/C-237` at `37ba8f7`.** The implementor was
  stopped by the operator while running the full gate (its last words: *"62 GB free again. Running the
  full gate."*), so the diff is **committed but neither reviewed nor gated** — the coordinator
  committed its working tree so it could not be lost to a later cleanup, and makes no claim about it.
- What is on the branch: **1,497 insertions across 8 files** — `crates/connectors-api/src/{api,exec,lib}.rs`,
  `src/index.html`, a rewritten `ui/test/host-page.test.mjs` (+612), and two new integration tests,
  `tests/catalogue_response.rs` (183) and `tests/dry_run.rs` (145).
- **To resume:** merge `main` into `impl/C-237` first — it is behind C-437 and whatever else has
  landed — then run the gate it never finished. Do not assume the diff is complete; the story's
  acceptance asks for a measured before/after request count, and no such measurement was reported.
- Deliberately **not** merged into the release that follows. An unreviewed, ungated diff touching the
  host's request path is not something to carry into a published version on the strength of it
  probably being fine.
- **2026-08-01, from the implementor after it was resumed — worse than the parking note knew.**
  - **There is no `BASE_PROOF` at all.** No failing-first run at the merge base was recorded.
  - **`crates/connectors-api/src/index.html` grew by 457 lines and its only test suite never ran.**
    `node --test` failed with `MODULE_NOT_FOUND` because a fresh worktree has no `node_modules`.
    Not a code failure, and **not a pass**.
  - The constraints that suite guards are the safety-relevant ones: `textContent` never
    `innerHTML`, POST-not-link for an auth change, and the dev button only under `status.dev`.
    **All three are unproven in this diff.**
  - Four Rust tests do pass — `tests/dry_run.rs` (2) and `tests/catalogue_response.rs` (2) — so the
    rail-data and dry-run half is real. The N+1 fix, the search, the credential removal and the
    layout work are not claimed by anything that ran.
- **Coordinator error, recorded because it cost something.** I reclaimed the implementor's worktree
  on the reasoning that committing to `impl/C-237` preserved the work. The *content* was preserved —
  `37ba8f7` is intact — but the agent was still resumable, and deleting the worktree ended its
  ability to finish. The rule I was applying (reclaim only what is integrated or branch-preserved)
  does not distinguish a live agent from a finished one, and it should have.
- **To resume:** fresh worktree, `git merge --no-ff main` (it is behind C-437 and C-432),
  `npm ci` in `crates/connectors-api/ui` **before** claiming any gate, then the base proof the story
  never had.

- **2026-08-02 — resumed, gated and finished.** Every item above is now ticked against a command run
  in the session that ticked it. What the resume actually consisted of:
  - **`main` merged in, not rebased**, so `37ba8f7` survives as the audit trail. The merge was
    **clean**: 283 files arrived from `main` and not one of them was under `crates/connectors-api/`,
    so nothing in the parked diff had to be reworked for it. The only overlap was this story file,
    which auto-merged. `git diff --stat main...HEAD` is 7 files, all inside the crate.
  - **The base proof the story never had.** Run in this worktree with its own `target/`, by
    restoring `crates/connectors-api/src/` to `dd8d21a` — the merge base — and leaving the tests in
    place. `ui/test/host-page.test.mjs` went **9 red of 14**, and the named one quoted the 30 URLs
    verbatim: *"opening a connector fetched 30 operation details to render a list the host already
    sent whole"*. `tests/catalogue_response.rs` (2) and `tests/dry_run.rs` (2) were red too — the
    first on *"`airtable-record-get` carries no `tool`"*, the second on the dry-run route not
    existing. The five sign-in and safety tests were green at the base and stayed green, which is
    the correct result for tests C-239 wrote about behaviour this story does not change.
  - **The parking note's three unproven safety constraints are now proven.** `npm ci` in
    `crates/connectors-api/ui` was all that stood between the suite and running; `textContent`
    never `innerHTML`, POST-not-link, and the dev button only under `status.dev` are Node tests 3,
    4 and 1, and all three pass.
  - **Nothing from `37ba8f7` was discarded.** Reviewed in full and kept as written; the backend
    half is well-argued and the page's comments carry their own reasoning. Two things were
    **added**: a fifteenth Node test for the layout-hygiene item, which nothing asserted (it reads
    `getComputedStyle` on rows the page really drew, and is red with the `min-width: 0` rule
    removed — verified), and an unconditional `println!` in `catalogue_response.rs` so the byte
    figure in `CEILING`'s doc comment is reproduced by a command instead of remembered.
  - **The response-size figure re-measured, and it holds.**
    `cargo test -p connectors-api --test catalogue_response -- --nocapture` →
    `GET /v1/connectors: 284623 bytes, 54 connectors, 679 operations`, against a 512 KiB ceiling.
    Identical to the parked implementor's number even after C-153's service tags landed, because
    tags do not reach `ConnectorView`.
  - **Gate, all in this worktree:** `cargo test -p connectors-api` 107 passed / 0 failed;
    `cargo test --workspace --no-fail-fast` 170 targets, **1568 passed, 0 failed**;
    `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all --check` clean;
    `npm test` in `crates/connectors-api/ui` **15 passed, 0 failed**.
  - **One thing to know when reading the diff.** The story's `note:` frontmatter says this phase
    *"needs no backend work"*. That is wrong, and the story's own Notes say so two sections down —
    *"the N+1 fix is a small backend change, not a client one"*. The diff does the backend change
    the Notes describe (five fields onto `OperationWiring`, `input_schema` onto `OperationView`, a
    `dry-run` route). The frontmatter note is the stale half; the body governed.
