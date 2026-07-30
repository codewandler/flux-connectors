---
id: C-48
title: Make the root documentation current and audience-specific
pillar: Surfaces
status: in-progress
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
- [ ] `README.md` leads with the current v0.1.0 capability boundary, explains the useful outputs,
      and includes a copy-pasteable local quick start.
- [ ] `README.md` distinguishes implemented commands from planned commands and keeps every known
      non-working runtime path explicit.
- [ ] `AGENTS.md` puts the required story workflow before repository detail and separates durable
      invariants, generated-file ownership, known intentional failures, and validation commands.
- [ ] The public start page leads with services, operations, safety metadata, and availability—not
      compiler architecture—and links to no internal design, roadmap, story, or agent document.
- [ ] The public site uses the canonical flux-connectors mark in its navigation, hero, and favicon;
      a test prevents the published copies from drifting from `assets/brand/`.
- [ ] Public `catalog.json` and rendered issue notices do not expose internal design or story
      references.
- [ ] Claims and examples agree with the current CLI, workspace manifest, and generated artifact
      plan; repository-local links resolve.
- [ ] The README checked-artifact test and the documentation site's build pass.

## Progress
- Review in progress. The working tree started clean. The current documents still say v0.0.1 even
  though the workspace is v0.1.0, and the README does not identify `check`, `fetch`, and `install` as
  unimplemented where it first lists them.

## Notes
- Scope expanded at the user's request from the root documents to the public documentation surface.
  The public catalogue schema changes only to remove internal `documentation` and `story` pointers.
