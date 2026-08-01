---
id: C-436
title: "A connector cannot name a single link — not its homepage, not its API reference, not a status page"
pillar: Spec
status: ready
priority: 2
design: docs/designs/connector-presentation.md
epic: connector-presentation
areas: [connector-spec, connector-cli]
note: "a provider publishes fifteen keys into catalog.json and not one is a link. Meanwhile `docs_url` already exists TWICE at lower scopes — on ConfigField and on ManualSetup — so a field can say where its documentation lives and a connector cannot"
---

# A connector cannot name a single link — not its homepage, not its API reference, not a status page

## Goal
Let a connector declare a typed list of resources — link, kind, title, optional description — so a
listing can show a person where to go, and a host can find the docs link specifically rather than
guessing at an entry.

## The gap, measured

A provider publishes fifteen keys into `web/public/catalog.json`: `id`, `vendor`, `description`,
`authority`, `api_version`, `base_url`, `hosts`, `runtime`, `services`, `operations`,
`operation_count`, `auth`, `config_choices`, `channels`, `events`. **None is a link.**

And the field already exists twice, both times below the connector:
`ConfigField::docs_url` (`crates/connector-spec/src/config.rs:778`) and `ManualSetup::docs_url`
(`crates/connector-spec/src/inbound.rs:366`).

## Acceptance
- [ ] A connector declares `[[resources]]` — a **link**, a **kind**, a **title**, and an optional
      description. A failing-first test declares one and asserts it reaches the IR and the published
      document.
- [ ] **`kind` is a closed set**, decided against what the 35 existing comment URLs actually point at
      (see [C-438](C-438-lift-the-comment-urls.md)) rather than guessed. An open set becomes 54
      spellings of "docs" and no consumer can switch on any of them — the same argument `Format` and
      `Risk` already make.
- [ ] **`docs_url` does not become a third spelling.** Either the connector level reuses the existing
      vocabulary or all three scopes become one resource list — state which and why. A field, a setup
      and a connector each saying "documentation" differently is the defect, not the fix.
- [ ] Resources reach the **manifest and `catalog.json`**, or nothing can render them.
- [ ] **Unstated is distinguishable from stated**, and neither reads as poor. This trap has now been
      hit by C-235, C-408, C-430 and C-433; a fifth is not bad luck.
- [ ] A URL is validated as a URL and refused otherwise — but **the gate does not fetch it**.
      `build`/`diff`/`check` are offline by contract, and a reachability check that lives in the gate
      either breaks hermeticity or quietly stops running.
- [ ] Decide whether resources belong on the connector, the service, or both, and say why. `google`
      is the live case: one vendor, several products with their own documentation, and `[[services]]`
      already carries `base_url` and `api_version` for exactly that reason.

## Progress
- (not started)

## Notes
- **This is presentation metadata and must never become load-bearing.** Nothing in the compile path,
  the credential path or the egress allowlist may read it. A `hosts` entry is a security claim; a
  resource link is not, and the two must not be confused because both contain a URL.
- `title` and `description` earn their place only where they differ from the kind and the connector's
  own description — `"Documentation"` on a `docs` resource is noise. Worth saying in the field docs so
  authors do not fill them reflexively.
- Sequenced before [C-438](C-438-lift-the-comment-urls.md), which needs the declaration to exist, and
  before [C-439](C-439-render-connector-presentation.md), which renders it.
