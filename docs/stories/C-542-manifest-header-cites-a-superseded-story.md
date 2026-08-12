---
id: C-542
title: "The emitted manifest header cites a superseded story"
pillar: Codegen
status: ready
priority: 3
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-cli, artifacts]
note: "seam.rs writes '# Auth and the http_hosts allowlist land in C-10.' into all 67 manifests; C-10 is closed as superseded (C-535), so the generated comment now points readers at a story that is never coming"
---

# The emitted manifest header cites a superseded story

## Goal

Stop every generated `.connector.toml` from promising C-10. The emitter comment at
`crates/connector-cli/src/seam.rs:597` — `# Auth and the \`http_hosts\` allowlist land in C-10.` —
predates Decision 0022; C-535 closed C-10 as superseded-never-implemented, so 67 committed
artifacts now open with a pointer to work that is not coming (found by C-535's independent review;
e.g. `connectors/zendesk.connector.toml:2`).

## Acceptance

- [ ] The emitted header comment states the honest current arrangement — auth is assembled by the
      host resolver (`connector-pack`), and the manifest becomes a projection of the catalog
      artifact under C-534's program — without naming a superseded story as pending.
- [ ] A failing-first test pins the new header text (the old text is the seeded failure).
- [ ] A full `build` regenerates all 67 manifests and `connectors.lock` consistently; `diff`
      reports clean afterwards and the artifact count claims in `README.md`/`AGENTS.md` are
      re-verified (the count does not change — only bytes inside existing artifacts).

## Progress

- (not started)

## Notes

- Write set: `crates/connector-cli/src/seam.rs`, `connectors/*.connector.toml` (67), and
  `connectors.lock` — collides with C-536 (`connector-cli`, the lockfile) and with any provider
  story; never share a wave with either.
- Filed 2026-08-12 from C-535's review finding.
