---
id: C-3
title: Load and validate provider TOML
pillar: Spec
status: ready
priority: 4
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec]
---

# Load and validate provider TOML

## Goal
Parse `providers/<name>.toml` into the IR with errors good enough to author against, covering both
roles the file plays: a pointer at a vendor spec, and a complete hand-authored connector definition.

## Acceptance
- [ ] A TOML that declares operations inline — with no vendor spec present at all — produces a
      complete, valid `Connector`. This is the "two front-ends, one IR" requirement.
- [ ] A TOML that only points at a spec source plus patches parses into the patch set for `C-6`.
- [ ] Validation rejects: unknown keys, an operation with no method or path, an auth purpose with no
      scheme, and a `basic` scheme missing `user_env`.
- [ ] Golden-file error snapshots for each rejection above — failing-first, since error text is the
      authoring interface.
- [ ] A documented JSON Schema for the provider TOML, kept in sync by a test.

## Progress
- (not started)

## Notes
- `deny_unknown_fields` everywhere: a silently ignored typo in a provider file is exactly how
  action-proxy's YAML drifted.
- No network in this crate — the loader takes bytes.
