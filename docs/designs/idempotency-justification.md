# Design: declaring the idempotency a `POST` actually has

> Story: [C-186](../stories/C-186-idempotent-post-cannot-be-declared.md) ·
> Guard: `crates/connector-flux/src/op.rs::check_write_metadata` ·
> Field: `connector_spec::Operation::idempotent_because` ·
> Loader: `crates/connector-spec/src/provider.rs::validate_idempotency_justification` ·
> Artifact: `web/public/catalog.json` (`crates/connector-cli/src/site.rs`)

## The defect, stated precisely

`check_write_metadata` refused `idempotency = "idempotent"` on `POST` and `PATCH` **by method,
regardless of endpoint semantics**. RFC 9110 §9.2.2 makes neither method idempotent, `idempotency` is
what tells flux whether wrapping a call in a `retry` is sound, and most `POST`s create something — so
the rule was right, and remains right, nearly every time.

It was also wrong for at least three shipped operations, each idempotent by its **vendor's** own
behaviour while its verb was not:

| operation | method | vendor behaviour | shipped as |
|---|---|---|---|
| `cloudflare-cache-purge` ([C-169](../stories/C-169-provider-cloudflare.md)) | `POST` | `purge_everything` names a target state, not a delta; a repeat empties an already-empty cache | `non_idempotent` |
| `launchdarkly-flag-toggle` ([C-175](../stories/C-175-provider-launchdarkly.md)) | `PATCH` | a single JSON Patch `replace` onto one environment's `on` bit, an absolute value | `non_idempotent` |
| `miro-sticky-note-update` ([C-183](../stories/C-183-provider-miro.md)) | `PATCH` | the note's whole content is sent as an absolute value | `non_idempotent` |

Each implementor did the only thing available: declare what the compiler accepted, and write the
truth in a comment. **The comment is not what a host reads.** `idempotency` travels to flux's
`ToolSpec`, and a host deciding whether a retry is safe reads the field.

Two things made this unlikely to be found by accident, and both are worth keeping in mind for the
next instance of the shape:

- **The direction of the error is safe.** An under-claim makes a host *more* conservative. Nothing
  breaks; a retry that would have been fine is simply never attempted, forever, in silence.
- **The prose was right and the code was wrong**, which is the inverse of C-151, C-152 and C-159.
  The lesson generalises past the direction: two statements of one fact drift, and only one of them
  was machine-checked.

### The measurement that says this will recur

[C-110](../stories/C-110-provider-linear.md) is the same guard hitting a whole connector rather than
one operation. Linear is GraphQL: every operation is a `POST` to one endpoint, so `check_write_metadata`
— which derives write-ness from the verb — forced **every** operation, including four pure reads, to
`risk >= medium` and `non_idempotent`. Review found that `idempotency` there carried **zero authored
information** while still reaching `ToolSpec`, so a host's retry logic read "do not retry" for four
reads that were nothing but reads. That connector was withdrawn for unrelated reasons
([graphql-vendors.md](graphql-vendors.md)), but the pressure returns with the next non-REST vendor.

## The decision

The story offered three options. **Option 2: keep the refusal, add an explicit escape that requires a
stated reason.**

Recorded reasoning, including why the other two were rejected:

**Option 1 — trust the author, drop the method rule.** Rejected. It is the cheapest change and it
deletes a guard that catches a real, common mistake: metadata copied from a read onto a write. The
repository has the evidence that the guard bites — `op_emitter.rs::a_post_may_not_claim_to_be_idempotent`,
`linear_connector.rs::a_graphql_read_cannot_declare_itself_idempotent`,
`webflow_connector.rs::site_publish_is_forced_non_idempotent_by_the_post_rule` and `dropbox`'s
whole-connector assertion all exist because someone might. Deleting the rule would make "safe to
retry" a claim any author could make by typing one word, with nothing to review.

**Option 3 — rename the field to mean "safe to retry per the method".** Rejected, though it is
legitimate and would have removed the misleading comments. Two objections. First, `idempotency` is
not this repository's field: it is `flux_spec::Idempotency`, reaching flux's `ToolSpec`, and
renaming a value away from its consumer's vocabulary would put a second meaning behind one name at
the exact boundary where a host reads it. Second, it answers the wrong question. A host wants to know
whether the *vendor* deduplicates; "per the method" is something the host can compute for itself from
the verb, so a field carrying only that carries nothing.

**Option 2 — the escape hatch.** Chosen. It preserves the guard for the case it was written for, and
it converts the deliberate case from unrepresentable into auditable. The cost is one optional field.

## What landed

### `idempotent_because`, on `Operation`

An `Option<String>` on the IR, `#[serde(default, skip_serializing_if = "Option::is_none")]` so that
landing it moves no `ir_sha256` for the operations that do not use it.

It is refused in three cases, each a distinct author mistake and each with a golden-file snapshot
under `crates/connector-spec/tests/golden/`:

| written | refused because | fixture |
|---|---|---|
| on `GET`, `PUT`, `DELETE` | nothing was refusing the claim there; a justification answers no refusal, and a field that means nothing in most places is read in none | `idempotency-justification-on-a-get` |
| beside `idempotency` that is not `idempotent` | the prose asserts what its own field denies — C-186's defect, arriving backwards | `idempotency-justification-without-the-claim` |
| shorter than 24 characters after trimming | an escape hatch that costs nothing is a deleted guard wearing the guard's clothes | `idempotency-justification-says-nothing` |

**On the 24-character floor.** It is a floor on *effort*, not on truth, and it is calibrated rather
than invented: 24 is the length of `"purging twice is a no-op"`, the shortest honest reason anyone
working on this story actually wrote. It is trivially gameable by anyone determined to game it, and
that is not the threat model — the mistake this catches is the author who wants the build to go
green, and who will not write a sentence they cannot defend once they know a reviewer reads it.

What no compiler can check is whether the sentence is **true**. That is what the artifact is for.

### It reaches an artifact

`AGENTS.md` records six declarable surfaces that the loader validates and no artifact carries, and
calls that the largest real gap in the repository. A declaration nothing can read is prose with a
schema attached — so publishing `idempotency: "idempotent"` on a `POST` while keeping the evidence in
a TOML comment would have reproduced C-186's own split one level up.

`web/public/catalog.json` carries it, as a sibling of the claim it licenses:

```json
{
  "id": "cloudflare-cache-purge",
  "idempotency": "idempotent",
  "idempotent_because": "purging an already-empty edge cache is a no-op: …"
}
```

`null` for every operation whose idempotency follows from its verb, which is almost all of them —
matching the document's existing rule that every key is always present and an absent value is `null`
(`site.rs`, "The published shape"). `SCHEMA_VERSION` is **not** bumped: the document's own rule is
that adding a field is additive for every consumer that reads by name.

Three surfaces do **not** carry it, deliberately:

- **`connectors/*.flux`.** `flux_lang::program::CompositeOpMeta` has a closed field set —
  `description`, `risk`, `idempotency`, `effects`, `expose`, `limits`, `view` — and no free-form
  slot. Carrying the reason would need a flux-lang change, and the pin is a crates.io version.
- **`crates/catalog/src/generated/<provider>.rs`.** A field on `catalog::Operation` must be written
  in every one of the 254 struct literals across 53 provider tables, because Rust struct literals are
  exhaustive. The reason is for a reviewer, not for the host's request path — the *value* already
  reaches `ToolSpec` through the existing field, which is the harm C-110 measured — so the cost is
  not worth paying now. If a host ever needs to render the justification, that is the change to make.
- **`connectors/*.connector.toml`.** The manifest carries operation **ids** and no per-operation
  metadata at all. `AGENTS.md` explicitly refuses ad-hoc widening of it; the surface-to-artifact
  mapping is [connector-surfaces.md](connector-surfaces.md)'s decision, not this story's.

### The guard is qualified, not removed

The emitter refuses `idempotent` on `POST`/`PATCH` **unless** a justification the loader has already
validated is present. Written in that direction on purpose: the claim is refused unless justified,
never silently corrected to `non_idempotent`, so a missing justification cannot degrade into a
plausible wrong declaration.

`check_write_metadata` also refuses `idempotent_because` on a method that never needed it, ahead of
its own `mutates` gate — the loader covers a provider file, and this covers an IR assembled in
memory, which is the only route the loader does not see.

**The `risk` half is untouched.** A state-changing method still may not declare `risk = "low"`,
unconditionally, with no escape.

## The full method matrix

The story asked for this to be confirmed rather than assumed. After this change:

| method | `risk = "low"` | `idempotency = "idempotent"` | `idempotent_because` |
|---|---|---|---|
| `GET`, `HEAD`, `OPTIONS` | permitted | permitted | **refused** |
| `PUT` | refused | permitted (RFC 9110 §9.2.2) | **refused** |
| `DELETE` | refused | permitted (RFC 9110 §9.2.2) | **refused** |
| `POST` | refused | **only with a justification** | permitted, required for the claim |
| `PATCH` | refused | **only with a justification** | permitted, required for the claim |

Note what `PUT`/`DELETE` "permitted" means and does not: the method is idempotent, so the *claim* is
allowed, but nothing obliges a connector to make it. `cloudflare-dns-record-delete`,
`miro-sticky-note-delete` and `airtable`'s record delete all decline it, because each vendor answers
a repeat with `404` rather than a repeat of the first `200` and documents no guarantee. That is a
connector author declining to claim something they cannot back, and it is correct.

## Semver

`connector-spec` is published on crates.io as `codewandler-connector-spec`. This change has two
distinct audiences and they are affected differently:

- **A downstream author writing their own `providers/<id>.toml`: additive.** The loader's accepted
  input strictly widens. Every file that loaded before loads unchanged, and the new refusals can only
  fire on a file that uses the new key. Emitted output is unchanged for every operation that does not
  use it — verified: the only artifacts that moved are the three named above.
- **A downstream Rust consumer of the `connector_spec` API: breaking.** `Operation` is a public
  struct **without** `#[non_exhaustive]`, so adding a public field breaks struct-literal construction
  and exhaustive destructuring. Under Cargo's pre-1.0 rules that requires the minor slot: **0.7.x →
  0.8.0**.

**[C-231](../stories/C-231-nothing-stops-a-secret-field-gaining-an-example.md) already forces the
next release to 0.8.0, and nothing here is worse than that covers.** No published item is removed or
renamed; `Idempotency` is untouched; `check_write_metadata` only ever accepts strictly more than it
did. The one behavioural change a downstream consumer could observe is that an IR they construct in
memory with `idempotent_because` set on a `GET` is now refused at emission — but that field does not
exist for them to set until this version, so no existing code can reach it.

Worth filing separately rather than doing here: **`Operation` should probably be `#[non_exhaustive]`**,
which would make every future field additive. It cannot be added quietly — `connector-flux`,
`connector-cli` and this crate's own tests all construct `Operation` literally and are all *outside*
`connector-spec` for the purposes of that attribute — so it is its own change, on its own bump.

## What this does not fix

`risk` has the same method-shaped heuristic and no escape at all. `providers/notion.toml` records the
trade for `notion-database-query` and `notion-search`, two `POST` **reads** forced to `medium`, and
C-110 measured the whole-connector version of it for GraphQL. A `POST` that is a read is a real and
recurring shape, and this story deliberately did not widen into it: `risk` gates flux's *approval*
path, so relaxing it is a safety change and deserves its own story with its own evidence — not a
second clause in a change about retries.
