---
id: C-241
title: "Four Resend operations still send a bare versionless `User-Agent`, overriding the versioned identity C-223 gave every other connector"
pillar: Spec
status: done
priority: 3
design: docs/designs/host-identity.md
epic:
areas: [providers, bridge]
note: "left behind by C-223 deliberately and flagged by its review: removing the declaration needs `build --provider resend` plus whole-catalogue regeneration, which is coordinator-owned, and C-223's gate permitted zero red tests"
---

# Resend still declares a versionless `User-Agent`

## Goal

Let Resend inherit the versioned identity every other connector now sends, and remove the workaround
that predates it.

## What was measured

[C-223](C-223-the-host-sends-no-user-agent.md) made `connector-pack` send

```
User-Agent: flux-connectors/0.7.0 (+https://github.com/codewandler/flux-connectors)
```

on every request, with a connector's own declaration winning where it has one. Its review dumped all
299 shipped operations before and after: **295 gained exactly one header; 4 were unchanged.** Those
four are Resend's, because `providers/resend.toml:139` declares

```toml
const_headers = { "User-Agent" = "flux-connectors" }
```

That declaration was correct when it was written — it was the only way to satisfy a vendor that
refuses a request without one. It is now the *worse* of the two values available:

- it carries **no version**, so a vendor cannot tell 0.7.0 from a release two years from now;
- it is the bare product word C-223's own acceptance rules out — *"a `User-Agent` that lies is worse
  than one that is absent"* applies to one that says nothing, too;
- it is a per-connector workaround for a gap the host has since closed centrally, which is the shape
  C-214 is an instance of: one rule spelled in two places.

## Why C-223 did not remove it

Recorded rather than left to be rediscovered: removing the declaration changes a provider file, which
requires `cargo run -p connector-cli -- build --provider resend` and then leaves whole-catalogue
artifacts stale. Those are **coordinator-owned** (`AGENTS.md`, "Whole-catalogue artifacts are
coordinator-owned"), and C-223's gate permitted **zero** red tests. Doing it there would have meant
either a red gate or an implementor regenerating an artifact that is not theirs to write.

So this is a small story that must ride a wave which owns the catalogue regeneration — not a defect
in C-223.

## Acceptance

- [x] **Failing-first test:** a Resend operation carries the versioned identity. It carries the bare
      word today. Name it.
- [x] `const_headers` is removed from `providers/resend.toml`, and the header comment explaining why
      Resend needed one is rewritten to say the host now supplies it — not deleted, because the
      *vendor's* requirement is still a real fact worth recording next to the connector it affects.
- [x] The **exactly-one-`User-Agent`** property still holds for Resend — the catalogue-wide check
      C-223 added must stay green, and it is what proves the removal did not leave the operation with
      none.
- [x] `build --provider resend` and `diff --provider resend` are clean, and the whole-catalogue
      staleness failures this leaves are reported and resolved by the coordinator's full build rather
      than silenced.

## Notes

- **Check whether any other connector has since declared one.** At the time C-223 landed, Resend was
  the sole shipped `User-Agent` declaration — and `providers/github.toml` declares only `Accept`,
  correcting a claim in C-223's own story text. If a second has appeared, it belongs in this story.
- The vendor fact is the durable half: Resend rejects a request with no `User-Agent` and answers
  `403`, which is why this connector was the one that surfaced the gap at all. Losing that note would
  cost more than the workaround does.
- Cheap to do, and it should ride the next wave that already regenerates the catalogue rather than
  earning a regeneration of its own.

## Progress

**Done on `impl/C-241`.** `const_headers` is gone from `providers/resend.toml`; all four Resend
operations now inherit `flux-connectors/<version> (+<repository>)` from `connector-pack`'s
`identify`.

**The check the Notes asked for, run: Resend was still the only one.** Six providers declare
`const_headers` — `github` (`Accept`), `pagerduty` (`Accept`), `klaviyo` (`revision`), `anthropic`
(`anthropic-version`), `notion` (`Notion-Version`) and `resend`. None of the other five names
`User-Agent`, so nothing else belonged in this story and the catalogue now declares zero. That also
re-confirms the correction this story records: `providers/github.toml` declares only `Accept`.

**Three test files moved, and two of them were not optional.**

1. `crates/connector-pack/tests/request.rs::resend_inherits_the_versioned_host_identity` — the
   failing-first test. Named connectors, loaded by name, driven from the per-provider artifacts
   `build --provider resend` writes. Asserts presence, uniqueness, equality with
   `DEFAULT_USER_AGENT`, and that the **first product token** carries a version — the last is the
   assertion the shipped bare word failed, since `flux-connectors` has no `/`.
2. `crates/connector-flux/tests/resend_connector.rs` — finding 5 inverted. It asserted the
   declaration and the emitted header were present; it now asserts **both are absent**, because a
   stray header symbol left in a module would shadow the host identity on the wire while the
   provider file looked clean. The vendor's `403` fact is kept in the doc comment, which is where it
   stays true.
3. `crates/connector-pack/tests/request.rs::a_connector_declaring_its_own_user_agent_wins_and_gains_no_second_one`
   — **C-223 anchored this on Resend as "the shipped case", and this story removes the only shipped
   case there was.** Both halves are now fixtures doctored out of `github-issue-get`'s real emitted
   `Accept` header, in the `User-Agent` and `user-agent` spellings. The claim is unchanged; only the
   evidence moved, because pinning it to whichever connector declares one next would make the
   premise a coincidence.

**Four whole-catalogue staleness tests are red, not the three `AGENTS.md` tabulates** for a story
that only changes an existing provider. All four are one cause — committed `web/public/catalog.json`
does not match a rebuild — and all four are coordinator-owned:

| red test | binary |
|---|---|
| `the_committed_tree_is_a_fixed_point_of_a_build` | `connector-cli::catalog_artifacts` |
| `a_build_plans_both_readme_images_and_they_are_current` | `connector-cli::readme_snippet` |
| `the_build_writes_and_checks_site_catalog_json` | `connector-cli::site_catalog` |
| `every_shipped_operation_carries_its_metadata_and_its_flux` | `connector-cli::site_catalog` |

The fourth is the one the table omits. It compares the **Flux text** `catalog.json` carries against
what the emitter produces, so it goes red whenever a change alters an operation's emitted module —
here, the removed `User_Agent` line. `AGENTS.md`'s three-red figure was measured by editing a
`description` in `providers/zendesk.toml`, which also alters emitted Flux, so the table looks
under-counted by one for *any* existing-provider change rather than by something specific to this
story. Worth re-measuring and correcting there; not corrected here, because `AGENTS.md` is shared
and this story is not its owner.

`build --provider resend` wrote 5 artifacts and `diff --provider resend` reports
`7 artifacts up to date (1 provider checked)`.
