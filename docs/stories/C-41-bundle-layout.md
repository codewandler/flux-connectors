---
id: C-41
title: Move build output to a per-provider bundle directory
pillar: Build
status: backlog
design: docs/designs/connector-bundle.md
epic: connector-bundle
areas: [connector-cli]
note: breaking layout change — C-13, C-27 and C-33 all assume the flat shape
---

# Move build output to a per-provider bundle directory

## Goal
Group each connector's artifacts into one directory, so "the bundle" is a real thing a consumer can
copy, rather than files sharing a name prefix.

## Acceptance
- [ ] Build output becomes `connectors/<provider>/` containing the `.flux` module, the manifest, the
      markdown page, and `icons/`.
- [ ] **The `.flux` remains the single installable unit** — the bundle groups artifacts, it does not
      change what gets installed into `~/.flux/flows`.
- [ ] Discovery, writing, `diff` and `check` all follow the new layout; C-13's byte-identical-no-op
      and atomic-write guarantees still hold.
- [ ] The move is done in one change, not incrementally — a half-migrated layout is worse than
      either shape.

## Progress
- (not started)

## Notes
- **This is a breaking change to a shape three stories already assume.** C-13's discovery, C-27's
  writer and C-33's doc checks are all written against flat `connectors/<name>.flux`. That is why it
  is a separate story rather than folded into C-39 or C-31 — the churn should land once, deliberately.
- Sequence it **after** C-31 (markdown) and C-39 (synthetic ops), so the directory is created once
  with its full contents rather than migrated twice.
