---
id: C-415
title: "Vendor the five babelforce manager specs, scrubbed and provenanced"
pillar: Build
status: done
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [specs, connector-cli]
note: "the blocker providers/babelforce.toml:7-15 has recorded since C-17, resolved: the spec bytes are publishable, the GitLab fetch configuration is not — and the Testers Inc. accessId/accessToken examples come out"
---

# Vendor the five babelforce manager specs, scrubbed and provenanced

## Goal
Put the five babelforce OpenAPI documents under `specs/babelforce/` with their provenance recorded and
nothing in them that a public repository must not carry — so `[spec]` has something to point at.

## Acceptance
- [x] Five documents land under `specs/babelforce/`: `manager`, `user`, `auth`, `task-automation`,
      `task-schedule`, taken from `~/babelforce/projects/manager/manager-sdk/specs/` — i.e. **after**
      that repo's `pull.sh` has normalized `servers:` to the public production host and applied its
      four generator-compatibility fixes.
      → `specs/babelforce/<name>-<pull date>.openapi.yaml`;
      `vendored_specs.rs::the_five_babelforce_documents_are_vendored`
- [x] **The credential-shaped examples are gone.** `accessId: 036fea61-…` and
      `accessToken: 829b0c86…` for the `Testers Inc.` account appear at `user.openapi.yaml:293` and
      `manager.openapi.yaml:25935` in the source. A test greps the vendored bytes for a 32-hex literal
      and a bare UUID under an `accessId`/`accessToken` key and fails on either.
      → `no_credential_shaped_example_value_survives` (shape) and `no_scrubbed_literal_can_ever_reappear`
      (exact, by digest). The second exists because the first is not sufficient: the `accessId` value is
      reused as a plain `id:` three lines above, where a key-scoped rule cannot see it.
- [x] **No internal marker reaches the repo.** A test applies
      `manager-sdk/scripts/leak-markers.regex` (the internal forge host, internal repository paths
      and non-production hostnames) to
      everything added here and fails on a hit. `sources.json` and `pull.sh` are **not** copied in —
      they hold the GitLab host and project ids and are the thing that must stay internal.
      → `no_internal_marker_survives_in_a_vendored_document`,
      `every_url_in_a_vendored_document_points_at_a_public_host`, `no_pull_configuration_is_vendored`
- [x] The scrub is a **script in this repo**, not a manual edit — `scripts/`, runnable, so re-vendoring
      is reproducible and reviewable as a diff. → `scripts/vendor-babelforce-specs.sh`
- [x] Provenance is recorded per document: `sha256` of the vendored bytes and a version. `info.version`
      is `0.0.0-dev` on three of the five, so the file name carries the pull date and the sha256 is the
      real identity. `source_url` is omitted rather than pointing at an internal host — `SpecSource`
      already makes it `Option`. → `specs/babelforce.provenance.toml`;
      `provenance_records_every_vendored_document_and_its_hash_matches`,
      `a_provenance_entry_is_spec_source_shaped_and_names_no_internal_url`
- [x] `AGENTS.md`'s vendoring policy states the split in one paragraph: pulled bytes are vendored here,
      pull configuration is not. → "Vendored specs: the pulled bytes, never the pull configuration"
- [x] **Added at coordinator review, from this story's own Goal** ("nothing in them that a public
      repository must not carry"): no email address or telephone number that identifies a person or an
      internal system survives. → `no_personal_identity_survives_in_a_vendored_document`

## Progress
- **Done.** Six literals scrubbed across the five documents — 3 credential, 2 address, 1 number —
  in 19 changed lines. Full gate green; `flux-connectors diff` still reports
  `557 artifacts up to date (53 providers checked)`, so nothing generated moved.
- **The scrub rule.** A literal is discovered from the source and replaced *wherever it occurs, under
  any key*; nothing is hardcoded, so no secret, address or number is written into this repository —
  not even into the thing that removes them. Three classes:
  1. **Credentials** — a hex-and-dash value of 16+ characters under `accessId`/`accessToken`/`token`,
     replaced by the same literal with every hex digit zeroed.
  2. **Addresses** — any email not on the allowlist (`support@babelforce.com`, which is
     `info.contact.email`, plus anything at an RFC 2606 reserved domain), replaced by
     `redacted@example.com`.
  3. **Numbers** — any phone-keyed value that is not one of the constructed `+49 30 0000 00xx`
     examples, zeroed.
  All three are **allowlist-shaped**, so a value a future pull introduces comes out by default.
- **`Testers Inc.` / `Will Tester` / `firstName: Will` are deliberately KEPT.** Once the address and
  the number are gone these are fixture labels with nothing contactable behind them: `Tester` and
  `Testers Inc.` are self-evidently constructed and a bare first name identifies nobody. Removing
  them would cost the examples their readability and buy no privacy. Recorded so the decision is
  reviewable rather than merely observable — it is cheap to overrule, being one more discovery pass.
- **The denylist is spelled in digests.** `specs/babelforce.provenance.toml` records the SHA-256 of
  each scrubbed literal with its `kind`, and the test refuses that literal's return under any key.
  A digest is publishable and its preimage is not, which is the only reason an exact gate can exist.
- **Every gate was mutation-tested, not just observed green.** Reinserting the `accessId` literal
  under a plain `id:` turns the digest gate red while the shape gate stays green — the evidence that
  the two are complementary. Reinserting `will+test@babelforce.com` turns three gates red. Injecting
  an internal host turns both leak gates red.
- **`X-Auth-Access-*` is absent from these documents entirely** — see the correction under Notes.
- **`providers/babelforce.toml` is deliberately unchanged.** Rewiring it to `[spec]` is C-416's job.
  Its long comment block is now stale in its premise (the document *is* vendored) but correct in its
  conclusions; updating it belongs with that rewiring.
- **Latent trap for C-410/C-416** (found here, not fixed here): `discovery.rs` treats *every* file in
  `specs/<provider>/` as a spec document and `Provider::spec()` takes the **last by file-stem order**,
  which for babelforce is `user-2026-06-25`, not the manager document. Inert today — `seam::load`
  reads the TOML's `[spec]`, not discovery's — and squarely inside what C-410 ("many documents") has
  to solve. It is also why provenance sits at `specs/babelforce.provenance.toml` rather than inside
  the directory: a sidecar in there would be discovered as a sixth "spec".

## Notes
- **CORRECTION, measured while implementing: `X-Auth-Access-*` is not in these documents at all.**
  This story and `providers/babelforce.toml:75-96` both assume the pair is declared in
  `securitySchemes` alongside oauth2, and that ingest must keep *seeing* it so drift-check keeps
  reporting on it. All five documents declare **only** `oauth2` — the maintainers appear to have
  finished the scrubbing the inventory recorded as under way. Nothing was removed by our scrub: the
  pair was already gone upstream.
  - What this changes: "ingest must keep seeing the scheme" is **unsatisfiable**, so drift-check
    cannot report on the pair from these documents. The connector's refusal to model it is unaffected
    and still correct — it is now enforced by upstream's silence rather than by our overlay.
  - What survives, and what the gate therefore pins instead: the `accessId`/`accessToken` **schema
    properties** on the account payload, plus their `required` entries, in the `manager` and `user`
    documents. `the_declarations_survive_the_scrub` asserts exactly those, so "values are scrubbed,
    declarations are not" is still proven — as far as the documents permit.
  - Worth confirming with the babelforce API owners before C-416 assumes drift-check covers the pair.
- **Do not "fix" the deprecated header pair back in** if a future pull reintroduces it. Only example
  **values** ever come out; the declaration, if it returns, stays.
- `leak-markers.regex` explicitly excludes `X-Auth-Access-*` because the header *names* are public API.
  The example values are a different question and are what this story removes.
- **The scrub was widened at coordinator review** beyond credentials, to email addresses and telephone
  numbers. Neither is a secret; both are things a public repository must not carry, and the story's
  own Goal — "nothing in them that a public repository must not carry" — already covered them. The
  two addresses were `will+test@babelforce.com` (a named individual) and
  `trautomations@…​.iam.gserviceaccount.com` (an internal GCP service account, which names a project
  as well as a role).
- Confirm-and-rotate with the babelforce API owners is worth asking for separately. It is **not** a
  gate on this story — the values leave this repo either way.
- No Rust changes. This story is data plus a script plus a policy paragraph, which is what makes it
  safe to run beside C-4 and C-413.
