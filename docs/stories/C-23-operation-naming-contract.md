---
id: C-23
title: Make operation names a stable public contract
pillar: Codegen
status: backlog
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec, connector-flux]
note: op names are what users and models call — renaming one silently breaks callers
---

# Make operation names a stable public contract

## Goal
Pin how an operation gets its name, so a regeneration never silently renames a tool that flows,
prompts, and users already call.

## Acceptance
- [ ] The naming rule is documented and implemented in one place: a provider-scoped, dotted,
      lowercase form (`zendesk.ticket.show`), derived from an **explicitly declared** name in the
      provider config.
- [ ] A name is **never** derived from a volatile spec field (`operationId`, tag ordering, path
      position) without a pinned override — vendors renumber and re-tag specs freely.
- [ ] Regenerating from an unchanged config produces byte-identical names; a test asserts it.
- [ ] A name collision within a provider is a loud error, not a last-write-wins.
- [ ] Renaming an operation between builds is **detected and reported** by `flux-connectors diff`,
      because it is a breaking change to a published surface.
- [ ] Names are valid Flux identifiers and valid LLM tool names.

## Progress
- (not started)

## Notes
- Flagged as a risk in [connector-pipeline.md](../designs/connector-pipeline.md): "Op naming is a
  public contract. Names must be stable across regeneration."
- The failure this prevents is quiet and expensive: a vendor reorders its spec, an op is renamed,
  every flow calling it breaks at runtime with "unknown operation" — and flux *prunes* unresolvable
  composites rather than failing loudly (`prune_unresolvable`, C-117 in flux), so it fails silently.
