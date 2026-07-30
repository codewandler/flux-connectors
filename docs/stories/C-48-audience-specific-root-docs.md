---
id: C-48
title: Make the root documentation current and audience-specific
pillar: Surfaces
status: done
priority:
design:
epic: public-docs
areas: [docs]
note: README for humans · AGENTS for agents
---

# Make the root documentation current and audience-specific

## Goal
Give human readers a quick, honest path from “what is this?” to trying the compiler, give public-site
visitors a consumer-facing catalogue rather than a mirror of internal designs, and give agents an
operational contract whose mandatory workflow and generated-file boundaries are hard to miss.

## Acceptance
- [x] `README.md` leads with the current v0.1.0 capability boundary, explains the useful outputs,
      and includes a copy-pasteable local quick start.
- [x] `README.md` distinguishes implemented commands from planned commands and keeps every known
      non-working runtime path explicit.
- [x] `AGENTS.md` puts the required story workflow before repository detail and separates durable
      invariants, generated-file ownership, known intentional failures, and validation commands.
- [x] The public start page leads with services, operations, safety metadata, and availability—not
      compiler architecture—and links to no internal design, roadmap, story, or agent document.
- [x] The public site uses the canonical flux-connectors mark in its navigation, hero, and favicon;
      a test prevents the published copies from drifting from `assets/brand/`.
- [x] Public `catalog.json` and rendered issue notices do not expose internal design or story
      references.
- [x] Claims and examples agree with the current CLI, workspace manifest, and generated artifact
      plan; repository-local links resolve.
- [x] The README checked-artifact test and the documentation site's build pass.

## Progress
- Done. The root README now gives humans an honest v0.1.0 quick start and command matrix; AGENTS
  front-loads the story workflow and records source/generated boundaries.
- The public site was recast as a consumer catalogue, branded with the canonical mark, and swept in
  rendered desktop and mobile views. Public JSON schema v2 removes internal design/story pointers;
  tests pin that boundary and keep the published brand assets in sync.
- A second rendered-page sweep simplified availability language, removed irrelevant page-to-page
  navigation, made the parameter table mobile-safe, and removed implementation terminology from
  public issue summaries.

## Notes
- Scope expanded at the user's request from the root documents to the public documentation surface.
  The public catalogue schema changes only to remove internal `documentation` and `story` pointers.
