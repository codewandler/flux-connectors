---
id: C-516
title: "Connector operation direction is explicit at the ToolSpec seam"
pillar: Bridge
status: done
design: docs/designs/operation-direction.md
epic: all-integrations-connectors
areas: [connector-spec, connector-flux, connector-pack, metadata, safety]
note: "829 operations now carry reviewed read/write direction; stable spec identities fail closed and Flux remains the sole gather-admission authority"
---

# Connector operation direction is explicit at the ToolSpec seam

## Goal

Make vendor-state direction an explicit connector fact, independent of HTTP method, and project it
unchanged into Flux's host-effect, intent, approval and staging contracts. Proven reads can then be
scheduled as concurrent gather work while every mutation remains ordered and approval-visible.

## What was measured

Re-measured from canonical `origin/main` commit `a1a252b6` on 2026-08-04:

```text
$ rg -o 'effects \[[^]]*\]' connectors -g '*.flux' | sed 's/^[^:]*://' | sort | uniq -c
    829 effects ["network"]

$ jq '[.providers[].operations[]] as $o | {total:($o|length), exposed:($o|map(select(.flux|contains("expose true")))|length), get:($o|map(select(.method=="GET"))|length), non_get:($o|map(select(.method!="GET"))|length), non_get_empty_semantic:($o|map(select(.method!="GET" and (.semantic_effects|length==0)))|length), non_get_idempotent:($o|map(select(.method!="GET" and .idempotency=="idempotent"))|length), semantic_nonempty:($o|map(select(.semantic_effects|length>0))|length)}' web/public/catalog.json
{"total":829,"exposed":369,"get":451,"non_get":378,"non_get_empty_semantic":376,"non_get_idempotent":9,"semantic_nonempty":2}
```

`crates/connector-flux/src/op.rs::mutates` currently equates method with direction, while
`metadata` emits `Network` alone for every operation. `connector-pack` copies that list into
`ToolSpec`; every operation also reports the read-shaped `NetworkFetch`/`ReadTarget` intent.

The method inference is demonstrably false. The vendored Babelforce contract at
`specs/babelforce/manager-2026-07-10.openapi.yaml:4305` defines a `GET` whose description is
"Flush dialer tasks". Its emitted `babelforce-flush-dialer` operation is currently
`low`/`idempotent`, has only `Network`, and is unexposed but still catalogued and explicitly
resolvable.

## Acceptance

- [x] The connector IR carries one closed, required vendor-state direction for every operation.
      Direction is authored or reviewed where connector truth lives and is not inferred from an
      HTTP method, operation name, description, risk, idempotency, semantic effect, exposure or
      authorization result. Unknown or omitted direction refuses before artifact emission.
- [x] Lowering emits exactly `[Read, Network]` for a proven gather and `[Write, Network]` for a
      mutation, preserving declaration order canonically. `connector-pack` copies these host effects
      into `ToolSpec` without reclassification, and manifest, embedded catalogue and public catalogue
      expose the same direction without inventing a competing vocabulary.
- [x] Concrete intents agree: proven reads use a read intent/target and mutations use a write
      intent/target for the same resolved destination. Permission subjects remain authorization and
      audit identities, never concurrency conflict keys. Existing custom-origin substitution stays
      fail-closed and byte-identical between the request and its subject/intent destination.
- [x] `Tool::staging_disposition` plus Flux's canonical consequence predicate classify every proven
      read as gather-eligible only when its risk is `Low`, idempotency is `Idempotent`, semantic
      effects are non-consequential and invocation intents are non-mutating. Any unknown,
      inconsistent, argument-dependent, approval-requiring or consequence-bearing operation remains
      ordered/captured. Connector code does not duplicate a weaker C-528-only classifier.
- [x] All mutations are visible to `Effect::Write`-based approval. Whole-catalogue tests prove no
      write-shaped direction can carry read-shaped effects/intents or bypass the canonical
      consequence predicate, even when it is unexposed or explicitly resolved outside the model
      registry.
- [x] `babelforce-flush-dialer` is explicitly a mutation despite `GET`; its risk, idempotency,
      effects and intents become coherent. A one-fact adversarial fixture proves changing only its
      method cannot turn it into a read or make it gather-safe.
- [x] Non-GET reads such as Dropbox folder list, Notion search, Slack conversation history and
      SendGrid email validation remain conservatively ordered until individually reviewed and
      declared as reads; POST alone never becomes evidence of either direction.
- [x] The nine currently non-GET `Idempotent` operations are reconciled with Flux's canonical
      consequence contract. No write remains `Idempotent` merely because HTTP says PUT/DELETE is
      repeatable; protocol-specific replay safety uses the existing conditional/non-idempotent
      vocabulary with an authored condition unless the upstream canonical contract is deliberately
      amended and proven first.
- [x] Failing-first whole-catalogue tests cover all 829 operations and fail on omitted direction,
      Read/Write effect mismatch, read/write intent mismatch, mutating GET promotion, argument-mode
      promotion, approval bypass and any operation Flux's consequence predicate classifies
      inconsistently. A C-528 compatibility fixture proves safe connector reads become parallelizable
      only through the canonical contract and all others remain serialized.
- [x] Generated connector modules, manifests, embedded/public catalogues, `connectors.lock`, docs and
      the public explorer are regenerated from source and form a fixed point. The full Rust gate plus
      both Node consumer gates pass; counts and claims are re-measured in the completion evidence.

## Progress

- 2026-08-04: Filed from the read-only C-528 connector-effect audit and independently re-measured at
  canonical `origin/main` `a1a252b6`. Flux C-528 remains correctly fail-closed: `Network` without
  `Read` is consequence-bearing, so no current connector operation may run concurrently until this
  contract is implemented.
- 2026-08-05: Reviewed and migrated all 829 published operations to an explicit closed `read` or
  `write` value; these are authored connector facts, not inferred defaults. The spec-backed values
  are keyed by stable service + vendor `operationId`, and a same-session reconciliation measured
  exact map/published-operation agreement: Asterisk 96/96, Babelforce 388/388, Microsoft Graph 4/4,
  Zendesk 35/35. Pre-composition adversarial tests swap only upstream methods for a mutating GET and
  a read-only POST without changing their direction, while an upstream `operationId` rename reports
  both the orphaned reviewed key and the renamed operation's missing direction.
- 2026-08-05: Full regeneration re-measured 829 operations: 451 reads, 378 writes, zero idempotent
  writes and zero direction/effect mismatches. `connector-pack` resolves a concrete request for each
  operation before comparing its permission subject and intent destination. The host dependency
  graph executes Flux Flow's public `statically_gather_safe` seam instead of maintaining a
  connector-local consequence classifier.
- 2026-08-05: Completion measurements were reproduced from the generated public catalogue:

  ```text
  $ jq '[.providers[].operations[]] as $operations | {total: ($operations|length), read: ($operations|map(select(.direction == "read"))|length), write: ($operations|map(select(.direction == "write"))|length), idempotent_writes: ($operations|map(select(.direction == "write" and .idempotency == "idempotent"))|length), bad_effects: ($operations|map(select((.direction == "read" and (.flux|contains("effects [\"read\", \"network\"]")|not)) or (.direction == "write" and (.flux|contains("effects [\"write\", \"network\"]")|not))))|length)}' web/public/catalog.json
  {"total":829,"read":451,"write":378,"idempotent_writes":0,"bad_effects":0}

  $ for provider in asterisk babelforce microsoft_graph zendesk; do map=$(awk '/^\[patch\.directions\./ {inside=1; next} /^\[/ {inside=0} inside && / = "(read|write)"$/ {count++} END {print count+0}' "providers/$provider.toml"); published=$(jq --arg provider "$provider" '[.providers[] | select(.id == $provider) | .operations[] | select(.spec_source != null)] | length' web/public/catalog.json); printf '%s map=%s spec_backed=%s\n' "$provider" "$map" "$published"; done
  asterisk map=96 spec_backed=96
  babelforce map=388 spec_backed=388
  microsoft_graph map=4 spec_backed=4
  zendesk map=35 spec_backed=35

  $ jq -r '.providers[] | .operations[] | select(.id == "babelforce-flush-dialer" or .id == "dropbox-user-me") | [.id,.method,.direction,.risk,.idempotency] | @tsv' web/public/catalog.json
  babelforce-flush-dialer  GET   write high non_idempotent
  dropbox-user-me          POST  read  low  idempotent
  ```
- 2026-08-05: The coherent repository gate passed. `cargo build --workspace` and
  `cargo test --workspace --no-fail-fast` completed with no failures; `cargo clippy
  --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` completed cleanly.
  `npm ci && npm run build && npm test` under `web/` passed all 48 tests, and `npm ci && npm test`
  under `crates/connectors-api/ui/` passed all 15 tests. The final compiler fixed-point check was:

  ```text
  $ cargo run -p connector-cli -- diff
  1102 artifacts up to date (55 providers checked)
  ```

## Notes

- Related Flux story: `flux/C-528`, which parallelizes independent native calls but must not infer
  connector safety from HTTP method, low risk or idempotency.
- C-155 deliberately left every host effect unchanged while adding the separate semantic tier. This
  story supersedes only that host-effect non-change because the later scheduler/approval audit proved
  direction is a required safety contract; semantic effects remain distinct.
- Likely source seams: `crates/connector-spec/src/provider.rs`,
  `crates/connector-flux/src/op.rs`, `crates/connector-pack/src/spec.rs`,
  `crates/connector-pack/src/tool.rs` and
  `crates/connector-pack/tests/metadata_coherence.rs`.
