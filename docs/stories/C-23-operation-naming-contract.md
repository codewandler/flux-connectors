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

> **Scope split (C-37).** This story is the **local** half: how `Operation.id` is spelled, that it is
> declarable in Flux, stable across regeneration, and collision-checked. The **global** half — the
> `com.zendesk.api/support/tickets:v2#show` address — is
> [C-37](C-37-global-addressing.md) / [global-addressing.md](../designs/global-addressing.md). They
> are complements: flux cannot declare an address, so both identifiers exist and must stay in step.

## Goal
Pin how an operation gets its name, so a regeneration never silently renames a tool that flows,
prompts, and users already call.

## Acceptance
- [ ] The naming rule is documented and implemented in one place, derived from an **explicitly
      declared** name in the provider config.
      **A dotted form is impossible — C-8 proved it.** `flux_lang`'s `decl_name` grammar
      (`parser.rs:685-710`) admits only ASCII alphanumerics, `_` and `-`, and flux's own composite
      loader agrees (`../flux/crates/flux-flow/src/composites.rs:340`, "is not filename-safe"). So
      `op zendesk.ticket.show` **cannot be declared**, even though a *call* to that name parses.
      Verified against both the pinned 0.37 crate and flux's 0.38 tree, so it is not a pin artifact.
      Pick the replacement form here — that decision is this story's whole point. Note the same
      charset is what OpenAI and Anthropic accept for tool names, so dots were probably wrong for the
      LLM-tool half too.
- [ ] A name is **never** derived from a volatile spec field (`operationId`, tag ordering, path
      position) without a pinned override — vendors renumber and re-tag specs freely.
- [ ] Regenerating from an unchanged config produces byte-identical names; a test asserts it.
- [ ] A name collision within a provider is a loud error, not a last-write-wins.
- [ ] Renaming an operation between builds is **detected and reported** by `flux-connectors diff`,
      because it is a breaking change to a published surface.
- [ ] Names are valid Flux **declaration** names (not merely valid call targets) and valid LLM tool
      names. C-8's emitter currently refuses an undeclarable id with an error naming this story;
      that guard should stay once the rule lands.

## Progress
- (not started)
- **Now on C-17's critical path**, discovered by C-27's wiring: `connector-spec` **accepts** a dotted
  op id and `connector-flux` **refuses** it. The spec crate's own test fixtures spell ids as
  `zendesk.ticket.show`, so a provider TOML written the way those fixtures read will load and then
  fail at emit. The two crates currently disagree about what a valid connector is, and this story is
  where that is settled.

## Notes
- Flagged as a risk in [connector-pipeline.md](../designs/connector-pipeline.md): "Op naming is a
  public contract. Names must be stable across regeneration."
- The failure this prevents is quiet and expensive: a vendor reorders its spec, an op is renamed,
  every flow calling it breaks at runtime with "unknown operation" — and flux *prunes* unresolvable
  composites rather than failing loudly (`prune_unresolvable`, C-117 in flux), so it fails silently.
