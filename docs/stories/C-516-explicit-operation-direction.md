---
id: C-516
title: "Connector operation direction is explicit at the ToolSpec seam"
pillar: Bridge
status: ready
priority: 1
design:
epic: all-integrations-connectors
areas: [connector-spec, connector-flux, connector-pack, metadata, safety]
note: "C-528 safety prerequisite — all 829 generated operations currently expose only Network, so reads cannot be certified for concurrent gather and writes are invisible to Effect::Write-based approval"
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

- [ ] The connector IR carries one closed, required vendor-state direction for every operation.
      Direction is authored or reviewed where connector truth lives and is not inferred from an
      HTTP method, operation name, description, risk, idempotency, semantic effect, exposure or
      authorization result. Unknown or omitted direction refuses before artifact emission.
- [ ] Lowering emits exactly `[Read, Network]` for a proven gather and `[Write, Network]` for a
      mutation, preserving declaration order canonically. `connector-pack` copies these host effects
      into `ToolSpec` without reclassification, and manifest, embedded catalogue and public catalogue
      expose the same direction without inventing a competing vocabulary.
- [ ] Concrete intents agree: proven reads use a read intent/target and mutations use a write
      intent/target for the same resolved destination. Permission subjects remain authorization and
      audit identities, never concurrency conflict keys. Existing custom-origin substitution stays
      fail-closed and byte-identical between the request and its subject/intent destination.
- [ ] `Tool::staging_disposition` plus Flux's canonical consequence predicate classify every proven
      read as gather-eligible only when its risk is `Low`, idempotency is `Idempotent`, semantic
      effects are non-consequential and invocation intents are non-mutating. Any unknown,
      inconsistent, argument-dependent, approval-requiring or consequence-bearing operation remains
      ordered/captured. Connector code does not duplicate a weaker C-528-only classifier.
- [ ] All mutations are visible to `Effect::Write`-based approval. Whole-catalogue tests prove no
      write-shaped direction can carry read-shaped effects/intents or bypass the canonical
      consequence predicate, even when it is unexposed or explicitly resolved outside the model
      registry.
- [ ] `babelforce-flush-dialer` is explicitly a mutation despite `GET`; its risk, idempotency,
      effects and intents become coherent. A one-fact adversarial fixture proves changing only its
      method cannot turn it into a read or make it gather-safe.
- [ ] Non-GET reads such as Dropbox folder list, Notion search, Slack conversation history and
      SendGrid email validation remain conservatively ordered until individually reviewed and
      declared as reads; POST alone never becomes evidence of either direction.
- [ ] The nine currently non-GET `Idempotent` operations are reconciled with Flux's canonical
      consequence contract. No write remains `Idempotent` merely because HTTP says PUT/DELETE is
      repeatable; protocol-specific replay safety uses the existing conditional/non-idempotent
      vocabulary with an authored condition unless the upstream canonical contract is deliberately
      amended and proven first.
- [ ] Failing-first whole-catalogue tests cover all 829 operations and fail on omitted direction,
      Read/Write effect mismatch, read/write intent mismatch, mutating GET promotion, argument-mode
      promotion, approval bypass and any operation Flux's consequence predicate classifies
      inconsistently. A C-528 compatibility fixture proves safe connector reads become parallelizable
      only through the canonical contract and all others remain serialized.
- [ ] Generated connector modules, manifests, embedded/public catalogues, `connectors.lock`, docs and
      the public explorer are regenerated from source and form a fixed point. The full Rust gate plus
      both Node consumer gates pass; counts and claims are re-measured in the completion evidence.

## Progress

- 2026-08-04: Filed from the read-only C-528 connector-effect audit and independently re-measured at
  canonical `origin/main` `a1a252b6`. Flux C-528 remains correctly fail-closed: `Network` without
  `Read` is consequence-bearing, so no current connector operation may run concurrently until this
  contract is implemented.

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
