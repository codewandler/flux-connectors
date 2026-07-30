---
id: C-11
title: Prove every generated module parses and analyzes
pillar: Codegen
status: ready
priority: 8
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-flux]
note: **load-bearing** · without it invalid Flux can be committed
---

# Prove every generated module parses and analyzes

## Goal
Make it impossible to commit a connector that flux cannot load: every generated `.flux` module must
parse **and** analyze against flux-lang in CI.

## Acceptance
- [ ] A test walks every generated module and asserts `flux_lang::program::Module::parse_str`
      succeeds and yields a `Module::Program` whose `ops` are non-empty.
- [ ] Analysis runs too, not just parsing — an op referencing an unknown operation or a mistyped
      argument fails the test. Parsing alone would let semantically broken Flux through.
- [ ] A deliberately corrupted fixture makes the test fail (failing-first proof the gate has teeth).
- [ ] The check runs in CI on every change, not only when codegen is touched.

## Progress
- (not started)
- **Placement decided by C-27's finding: this gate belongs in `connector-flux`, not `connector-cli`.**
  `connector-cli` has no `flux-lang` dependency and `connector-flux` re-exports none of it, so
  `flux_lang::program::Module::parse_str` is unreachable from the CLI without a manifest edit. C-27
  therefore pinned the module envelope by *shape* only (`#` comments, never `//`) and left the real
  gate to this story. Put it where flux-lang is already a dependency.

## Notes
- This is the single most important test in the repository. Everything else can be fixed forward; a
  connector that does not load is a broken tool catalog in someone's flux session.
- flux prunes unresolvable composites with an audit record rather than failing startup
  (`prune_unresolvable`, C-117 in flux) — which is exactly why *we* must catch it here. flux
  degrading gracefully means a broken connector would otherwise fail silently.
