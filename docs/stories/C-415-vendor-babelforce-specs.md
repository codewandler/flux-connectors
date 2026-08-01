---
id: C-415
title: "Vendor the babelforce OpenAPI documents, scrubbed and provenanced"
pillar: Build
status: in-progress
design:
epic:
areas:
note: "providers/babelforce.toml:75-96 and the response-schema block below it both say the same thing — the one authoritative description of this API is not in the repo, because it embeds a credential-shaped example block for a real test account and this repo is public. Vendoring is what unblocks nine response schemas, drift-check on the deprecated header pair, and a `[spec]` front-end for our own first-party connector"
---

# Vendor the babelforce OpenAPI documents, scrubbed and provenanced

## Goal

The five babelforce OpenAPI documents live under `specs/babelforce/`, scrubbed of the credential-shaped
example values and of every internal marker, with their provenance recorded — so the document that
`providers/babelforce.toml` has been deferring to in prose is finally in the repository, and the scrub
is a script anyone can re-run rather than a hand edit nobody can reproduce.

There is **no Rust behaviour change** here. It is data, a script, tests over that data, and a policy
paragraph. Nothing touches `crates/*/src/`.

## Acceptance

- [x] The five documents (`manager`, `task-automation`, `task-schedule`, `user`, `auth`) are vendored
      under `specs/babelforce/`, each named for its pull date.
- [x] **No credential-shaped example value survives.** The `accessId` / `accessToken` / stream `token`
      literals for the `Testers Inc.` account are replaced everywhere they occur, including where the
      same literal is reused as a plain `id`. Failing-first test:
      `crates/connector-spec/tests/vendored_specs.rs::no_credential_shaped_example_value_survives`,
      backed by `the_known_credential_literals_can_never_reappear` for the reuse case.
- [x] **The declarations survive the scrub**, so ingest keeps seeing them and drift-check keeps
      reporting on them: `components.securitySchemes` and the `accessId`/`accessToken` schema
      properties are untouched. Only values are scrubbed —
      `vendored_specs.rs::the_declarations_survive_the_scrub`.
- [x] **No internal marker survives**, per the upstream `leak-markers.regex`, and no URL in any
      vendored document points anywhere but a public babelforce host. Failing-first tests:
      `no_internal_marker_survives_in_a_vendored_document`,
      `every_url_in_a_vendored_document_points_at_a_public_host`.
- [x] **Pull configuration is not vendored.** `specs/babelforce/` holds the five documents and nothing
      else — no `sources.json`, no `scripts/pull.sh`, which is where the GitLab host and the project
      ids live. Test: `no_pull_configuration_is_vendored`.
- [x] The scrub is a **runnable script** under `scripts/`, so re-vendoring is reproducible and reviewable
      as a diff rather than a manual edit — `scripts/vendor-babelforce-specs.sh`.
- [x] Provenance is recorded for each document: the pull date and the `sha256` of the vendored bytes,
      plus the `sha256` of the unscrubbed upstream bytes (`LockEntry::upstream_spec_sha256`, C-25).
      `source_url` is **omitted** rather than pointed at an internal host. Tests:
      `provenance_records_every_vendored_document_and_its_hash_matches`,
      `a_provenance_entry_is_spec_source_shaped_and_names_no_internal_url`.
- [x] `AGENTS.md` states the policy in one paragraph: pulled bytes are vendored here, pull
      configuration is not — "Vendored specs: the pulled bytes, never the pull configuration".

## Progress

- **Done.** 933 745 bytes added: five documents (933 874 → 889 934 of it the YAML), the provenance
  sidecar, the scrub script, and the test file. Gate green; `flux-connectors diff` still reports
  `557 artifacts up to date (53 providers checked)`, so nothing generated moved.
- **The scrub rule**, in one sentence: a credential literal is the inline scalar value of a key named
  `accessId`, `accessToken` or `token` that is at least sixteen characters of hex digits and dashes,
  and every such literal is replaced *wherever it occurs, under any key* by the same literal with
  every hex digit zeroed and the result single-quoted. Twelve lines changed across two documents.
  The "under any key" half is not decoration: the `accessId` value is reused as the `customer.id`
  of the same example three lines above, so a key-scoped rule would have left the credential in the
  file under a different name. The script discovers the literals from the source and hardcodes none,
  so no secret is written into this repository — not even into the thing that removes them.
- **The denylist is spelled in digests.** `specs/babelforce.provenance.toml` records the SHA-256 of
  each scrubbed literal, and the test refuses that literal's return under any key in any document.
  A digest is publishable and its preimage is not, which is the only reason an exact gate can exist
  here at all.
- **All four gates were mutation-tested, not just observed green.** Reinserting the `accessId`
  literal under a plain `id:` key turns `the_known_credential_literals_can_never_reappear` red while
  the shape-based gate stays green — which is the evidence that the two are complementary rather
  than redundant. Injecting an internal host turns both leak gates red.
- **`providers/babelforce.toml` is deliberately unchanged.** Rewiring it to `[spec]` is a behaviour
  change needing ingest plus the `[[patch.operations]]` selection that reproduces the nine
  hand-authored operations. Its long comment block about why the document is absent is now stale in
  its premise but correct in its conclusions; updating it belongs with that rewiring, not here.
- **Latent trap for whoever does that rewiring** (found here, not fixed here): `discovery.rs`
  treats *every* file in `specs/<provider>/` as a spec document and `Provider::spec()` takes the
  **last by file-stem order**. With five documents that is `user-2026-06-25`, not the manager
  document a connector would be built from. Nothing consumes it today — `seam::load` reads the
  TOML's `[spec]`, not discovery's — so this is inert, but a multi-document vendor directory is a
  shape `discovery` has never had to represent. That is also why provenance lives at
  `specs/babelforce.provenance.toml` rather than inside the directory: a sidecar in there would be
  discovered as a sixth "spec".

## Notes

- Source material is another repository and is read-only: `manager-sdk/specs/`, already post-`pull.sh`
  (servers normalised to the public production host, four generator-compatibility fixes applied).
- `providers/babelforce.toml` is **not** rewired to `[spec]` here. That is a behaviour change — it
  needs ingest plus the `[[patch.operations]]` selection that reproduces the nine hand-authored
  operations — and it is a separate story.
- **The `X-Auth-Access-*` securityScheme is not in these documents at all.** `providers/babelforce.toml`
  describes it as declared in `securitySchemes` alongside oauth2; upstream now declares only `oauth2`
  in all five. The maintainers appear to have completed the scrubbing the inventory said was under
  way. What survives is the `accessId`/`accessToken` **schema properties** on the account payload, and
  those are what `the_declarations_survive_the_scrub` pins — so "ingest keeps seeing it" is satisfied
  as far as the documents allow. Worth confirming with the API owners before the `[spec]` rewiring
  assumes drift-check can still report on the pair.
