# Native-plugin migration ratchet

C-505 makes the native-adapter migration a retained, offline contract. It does not migrate an
adapter and it does not start either implementation. Wave stories C-499 through C-503 capture the
observations, publish replacements and add evidence one adapter at a time.

## The three committed records

- [`native-plugins.toml`](native-plugins.toml) is the permanent inventory. Its adapter rows are
  tombstones: removing a Flux crate never removes its row. `host-kit` and `pack-index` are separate
  `[[support]]` rows because they are not integrations.
- [`conformance-v1.schema.json`](conformance-v1.schema.json) is the closed captured-observation
  format. [`fixtures/equivalent-one-shot.json`](fixtures/equivalent-one-shot.json) and
  [`fixtures/runtime-refused.json`](fixtures/runtime-refused.json) prove the supported and explicit
  unsupported shapes.
- `publications/<adapter>.json` is added only after publication succeeds. It retains the release,
  connector commit, immutable artifact identity and digest, replacement operation addresses and
  migration-note path.

There is deliberately no authored `present`, `published`, `retired` or `conformant` boolean. The
checker derives presence from the supplied Flux checkout, parses the receipt, and computes the
conformance verdict from paired observations.

## What a conformance document freezes

The `surface` records every legacy/replacement operation and event pair. Operations carry input and
output JSON Schemas (including an explicit `null` when the legacy side publishes no output schema),
declared errors, host and semantic effects, risk, idempotency and normalized capability subjects.
Events carry their payload schema and the same effect/subject facts. `lifecycle` states one-shot,
stream or lease behavior and makes cancellation, stream items/terminal values and
acquire/renew/release semantics explicit.

Each case then carries the input, the Exchange runtime/topology, and captured legacy and Exchange
observations. A transcript can return a result, emit a declared error or event, refuse, stream,
cancel, acquire/renew/release a lease, and terminate. Both evidence identities retain the runner,
source commit, capture time and raw-capture SHA-256.

Capability subjects are the normalized public authorization requirement `{action, resource}`, not
one side's internal string. The legacy plugin currently reports `plugin.operation` while an HTTP
connector reports a resolved request URL; the wave-owned runner records the reviewed mapping and
keeps each side's raw capture behind the evidence digest. C-505 does not invent events the current
plugin manifest does not declare.

## Verdicts are derived

`connector_cli::migration::conformance_verdict` has four outcomes:

- `MissingEvidence`: an evidence identity or either side of a case is absent;
- `Unsupported`: Exchange explicitly returns a `runtime` or `topology` refusal;
- `Diverged`: both sides exist and their normalized public observations differ; or
- `Conformant`: every frozen case has paired, equal observations.

There is no skip field or ignored path. An expected `runtime_refused` observation is useful evidence
that Exchange failed closed, but it remains `Unsupported`; it never unlocks legacy deletion.

## Cross-repository release check

Run from a flux-connectors checkout, naming the exact Flux checkout being released:

```bash
cargo run -p connector-cli -- migration-check --flux-root ../flux
```

The command is offline and starts nothing from the Flux tree. It reads `plugins/Cargo.toml` and each
member manifest, recognizes explicit and Cargo-implicit `flux-plugin-*` binaries, checks support
classification, and fails on an unknown or duplicated member. A missing inventoried adapter fails
unless its conformance verdict is `Conformant` **and** its publication receipt validates.

The initial ratchet succeeds while every adapter is still present. This is intentional: C-505 is
complete when the inventory, format, comparator and deletion gate exist. C-495 remains open until
the final retained row is absent from Flux with evidence.

## Wave workflow

For its adapters, each wave performs these steps in fixed C-499 → C-500 → C-501 → C-502 → C-503
order:

1. Capture the complete legacy surface and raw observations with the Flux-owned runner.
2. Capture the matching execution through Exchange. An unavailable runtime/topology is recorded as
   a refusal and leaves the adapter nonconformant; it is never skipped.
3. Add `conformance/<adapter>.json`. The comparator must compute `Conformant` before retirement.
4. Publish the connector/runtime artifact and migration notes, then add
   `publications/<adapter>.json` in this closed shape:

   ```json
   {
     "format": "flux-connectors-publication/v1",
     "adapter": "slack",
     "connector": "slack",
     "release": "vX.Y.Z",
     "connector_commit": "<40 hexadecimal characters>",
     "artifact": {
       "identity": "<immutable published identity>",
       "sha256": "<64 hexadecimal characters>"
     },
     "replacement_addresses": ["com.slack.api:v1#operation"],
     "migration_notes": "docs/migrations/slack.md"
   }
   ```

5. Run `migration-check` against the Flux release tree before removing the adapter and pack-index
   entry. Evidence accumulates; no wave waits for a global cutover.
