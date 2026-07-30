# Note: the `plugins/zendesk` citation is no longer verifiable

**Status:** correction · **Affects:** `vision.md`, `connector-pipeline.md`, `connectors-v1.md`,
`provider-operation-inventory.md`, and the stories that cite them.

## What happened

Many documents in this repo cite `../flux/plugins/zendesk/src/main.rs` — "687 lines of hand-written
Rust for roughly seven operations" — as the motivating comparison for connectors, and
`provider-operation-inventory.md` §3 cites specific line numbers in it for Zendesk's operation set,
parameter names and auth form.

**That file no longer exists, and it was never committed to flux.** Verified at flux `v0.38.0`:

```
$ ls plugins/zendesk/            → No such file or directory
$ git ls-files plugins/ | grep zendesk   → (nothing)
$ git log --diff-filter=D -- plugins/zendesk/src/main.rs   → (no deletion commit)
```

No deletion commit exists because it was never tracked. When this repo was scaffolded it was
**uncommitted working-tree material** in the flux checkout, and it has since gone. Only a stale
`plugins/target/debug/flux-plugin-zendesk` binary remains.

Found by C-28 while chasing an unrelated percent-encoding citation.

## What is and is not affected

**Not affected — the operation sets themselves.** The Zendesk operations in
`provider-operation-inventory.md` and `providers/zendesk.toml` describe Zendesk's real public API.
They were read from a real implementation and are independently checkable against Zendesk's own
documentation. Nothing needs re-deriving.

**Affected — verifiability and the strength of one claim.**

1. **The `path:line` citations cannot be re-checked by anyone.** A future reader who tries to confirm
   "Zendesk's user half is `<email>/token`, see `main.rs:5-6`" will find nothing there. Treat those
   citations as provenance-of-reading, not as live references.
2. **The headline comparison is weaker than stated.** "A connector replaces 687 lines of Rust"
   describes something that was never part of flux's shipped source. The *argument* stands — a stdio
   plugin for a SaaS product is a large hand-written artifact, and flux ships several — but the
   specific number should not be quoted as though it were checkable.

## What to do

- **Do not** rewrite the operation sets. They are correct.
- **Do** re-ground the comparison on a plugin that actually exists in flux's tree if the claim is
  wanted in public-facing material.
- **Do** treat this as the general lesson: a citation into a *neighbouring working tree* is not a
  stable reference. flux cut `v0.38.0` in the middle of this repo's first day and five line numbers
  moved in the auth-seam design alone. Cite by symbol, and record the version read.
