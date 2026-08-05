---
id: C-517
title: "Repository agents use the installed Flux Board workflow"
pillar: Bridge
status: ready
priority: 0
areas: [agents, docs, tests]
note: "Fleet dogfood — replace private Track slash commands with copyable versioned flux board commands"
---

# Repository agents use the installed Flux Board workflow

## Goal

Make this repository's mandatory agent workflow executable by any Codex, Claude or Fleet worker
that has the released `flux` binary, without a private Track plugin or prompt-only slash commands.

## Acceptance

- [ ] Failing first, a focused contract test proves the current mandatory workflow still names
      `/track:*` commands that do not exist in a plain shell or in the installed Flux CLI.
- [ ] The Track marker in `AGENTS.md` keeps its repository policies but uses copyable
      `flux board --root .` discovery, query, transition, evidence, done, check and sync commands.
- [ ] The workflow distinguishes human output from the stable `--output json` agent API and shows
      where optimistic revisions and idempotency keys matter for mutations.
- [ ] No step assumes the Board is a datasource, a private plugin, a tmux pane or repository-specific
      IPC. The story Goal and Acceptance remain authoritative and generated board regions remain
      generated.
- [ ] The focused contract test executes every documented read-only command against a hermetic
      temporary Track-compatible fixture and proves every mutating example exists in the installed
      schema without changing this checkout.
- [ ] The changed documentation and focused test pass, followed by the ordinary repository wave
      gate at integration.
