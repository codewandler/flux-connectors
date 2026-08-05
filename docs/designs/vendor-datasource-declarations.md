# Design: vendor datasource declarations — a connector's data surface as a projection over its operations

**Status:** proposed · **Pillar:** Spec · **Epic:** `vendor-datasources`
([C-511](../stories/C-511-vendor-datasources-epic.md) … C-514) · **Authority:**
`../flux-roadmap/decisions/0006-datasources-are-declared-read-surfaces.md`, rules 5, 6, 10, 11 and 12

> Repository facts below were re-measured in this worktree on **2026-08-05**; flux symbols were read
> in `/home/timo/projects/flux`. Re-grep by symbol; line numbers move.

## Why

Decision 0006 resolves the one ambiguous owner cell in the family: **vendor-data Datasource
Definitions belong to flux-connectors.** Which entities a vendor exposes and how to read them is
true of the integration regardless of who runs it — the same argument that put operations, events
and channels here. Today the connector IR has **no datasource surface at all**, while the decision
records that seventeen of the eighteen official plugins declare datasources through the plugin
protocol that Milestone 5 removes. Without this surface, the migration program deletes the only
working vendor-datasource channel and replaces it with nothing; `C-501` and `C-502` acknowledged
that gap only in prose until this design and Decision 0006 rule 11 made it a program rule.

This design is deliberately **not** the catalogue datasource.
[connectors-datasource.md](connectors-datasource.md) is the datasource *about* connectors — the
compiled-in catalogue, queryable from a session as an indexed backend. This document owns the
datasource declarations *of* a connector: what a vendor's own data surface knows, declared in
provider TOML and executed, like everything else a connector declares, as admitted operations.

## The surface

A connector gains a sixth member kind: `[[datasources]]`. A datasource member is **a projection
over that connector's declared operations** — Decision 0006 rule 6, quoted because it is the whole
shape:

> There is no independent retrieval contract: every datasource read executes as an admitted
> operation, so Exchange constructs no request of its own and existing grant metadata governs reads
> with no new machinery.

Concretely, a member declares:

- **A name in the per-service member namespace.** `Connector::member_names_of`
  (`crates/connector-spec/src/ir.rs:1956`) returns what is then six kinds together; a cross-kind
  collision stays a loud load error, exactly as [connector-surfaces.md](connector-surfaces.md)
  demands of any new surface. It renders into the same `…#name` address form as every other member.
- **An entity set whose schema is derived from the IR** — the backing operations' declared
  response schemas — and never hand-written. A hand-written entity schema would be a third place
  the vendor's shape is stated, and the two existing places already disagree often enough to have
  their own drift checks.
- **Per-verb operation bindings.** `list` names a declared operation of the same connector plus
  explicit parameter, filter, cursor and field mappings; `get` names a declared operation plus the
  mapping from the record id to that operation's id parameter. A verb the vendor cannot serve is
  omitted, not stubbed.
- **Cursor and paging vocabulary on the binding.** How a listing pages — cursor parameter,
  next-cursor pointer, page bounds — is a fact of the binding, stated where the read is declared.
  This **supersedes the dead `quirks.pagination` surface**, which is removed rather than left as
  another declared-but-unreachable shape (see below). The one-shot cursor spelling already ships
  in this repository — `Pagination::Cursor`'s `cursor_param`, `next_cursor_pointer` and
  `max_pages` (`crates/connector-spec/src/ir.rs:524`) — and **C-512 fixes it; the runtime-binding
  vocabulary [C-497](../stories/C-497-declare-runtime-operation-bindings.md) defines must not
  mint a second spelling of it**. C-512 waits on C-497 only for the stream/tail/lease terms, which
  datasource v1 does not use. This cursor is the paging of one `list` read — not the poll-channel
  *cursor operation* (`ChannelBinding::cursor`), which names an operation a poll transport calls
  on a schedule.
- **Credential reach: the backing operation's declared auth, and nothing else.** A datasource
  member never names a credential value, never introduces an auth declaration of its own, and
  reaches secrets only in the sense that its backing operation already does. Cursors served to
  callers are Exchange-minted opaque continuation tokens carrying no credential material (rule 7).
- **Read verbs bind read operations.** A `list` or `get` binding that names a
  `direction = "write"` operation is refused at load, naming the member and the operation. The
  check reads the resolved `Operation::direction` (C-516) — never `patch.directions`, which is
  ingest input the resolver has already folded into that field.

### Who consumes it

Exchange binds a published datasource member to a tenant connection label as a tenant Datasource
(rule 7) and serves schema/list/get through its existing admission gate; Flux reads it only through
the embedded Exchange client (rule 8), so Exchange unavailable means vendor datasources unavailable
— no local vendor adapter, no local index fallback. Nothing in this repository executes a read.

## Artifact reach — non-empty from the first release

[connector-surfaces.md](connector-surfaces.md)'s central finding is that a quarter of the declared
connector surface reaches no artifact. Decision 0006 rule 6 makes declared reach an **entry
criterion** for this surface: from its first release, `[[datasources]]` reaches

- **M** — the service manifest (`connectors/<provider>[-<service>].connector.toml`),
- **J** — the public catalogue (`web/public/catalog.json` and the published `v1` data), and
- **R** — the embedded Rust catalogue (`crates/catalog/src/generated/<provider>.rs`),

and **never F**, the generated `.flux` module. A datasource emits no `op`: the module is what a
Flux session loads, and the read seam is Exchange's, not the module's — the same split `events`,
`channels`, `config` and `verify` already hold.

## The `HashDomain` answer

`Connector::hash_domain` (`crates/connector-spec/src/ir.rs:2182`; the `HashDomain` struct is at
`ir.rs:2207`, and the exhaustive destructuring is in `HashDomain::of` at
`ir.rs:2257`) forces every new field to be classified as compiled
meaning or not. **`datasources` is compiled meaning and joins the hash domain.** Unlike the four
dead surfaces connector-surfaces.md catalogues, this one reaches three artifacts from its first
release, so the claim "editing it moves `ir_sha256` because a generated artifact changed" is true
rather than aspirational — a datasource member whose binding or cursor mapping moved is a connector
that changed, and `connectors.lock` must say so.

## Enforcement — declared surfaces are enforced, not decorative

Decision 0006 rule 12 exists because the plugin-manifest datasource failure — declared, dropped at
load, display-only at refresh, contributions unchecked — must not recur. Three gates, in order:

1. **Connector build time (this repository).** The loader refuses a binding to an operation the
   connector does not declare, a mapping to a parameter the operation does not take, a cursor
   pointer into a response the operation does not declare, a `get` binding with no id mapping,
   and a `list`/`get` binding naming a `direction = "write"` operation — read from the resolved
   `Operation::direction`, never `patch.directions`.
   `flux-connectors build` (`cargo run -p connector-cli -- build`) therefore fails loudly on a
   dangling projection, in the same pass that already refuses a dangling channel reply.
2. **Exchange bind time.** A tenant Datasource binding references a *published* connector
   datasource member, never a free-form kind string — the `exchange/X-108` hardening.
3. **Flux registration.** Flux validates the surface once when the granted tenant Datasource is
   registered through its live registration seam.

## The pattern generalizes — named once, designed later

Decision 0006 states the declared-surface pattern this design instantiates: Flux owns the contract
and the fixed tool surface; connectors declare the vendor mapping as a projection over that
connector's operations; Exchange binds it per tenant and executes every mutation as an admitted,
granted operation. The **datasource is the read instance; the board is the write-capable
instance.** A future connector *board* member declares a vendor status↔state mapping and per-verb
operation bindings as a connector fact, exactly parallel to this surface. It is named here so the
vocabulary is settled, and it is designed later, with Milestone 3 — nothing in this design or its
stories builds it.

## Non-goals

- **No independent retrieval contract.** Every read is an admitted operation; nothing here defines
  a second request path, and Exchange constructs no request of its own.
- **No `.flux` emission.** The generated module carries `op` declarations only; a datasource member
  reaches M, R and J and never F.
- **No streaming before the Milestone 3 vocabulary.** Datasource v1 is one-shot list/get with
  opaque cursors (rule 10). Tail and incremental-stream reads become a declared datasource-member
  capability with lease-owned lifetimes when Milestone 3's stream and lease vocabulary exists —
  C-497 is where those terms are defined for this repository.
- **No local execution and no local index.** Flux reads vendor datasources only through the
  embedded Exchange client; program-local indexed datasources stay Flux-local and are unaffected.
- **No credential values, anywhere.** A member reaches credentials only as its backing operation's
  declared auth; ingest-facing invariants (credential-marked material never enters a record) are
  Exchange's and Flux's to run, but nothing declared here may contradict them.

## `quirks.pagination` is superseded, and removed

Re-measured 2026-08-05 in this worktree: `Quirks::pagination`
(`crates/connector-spec/src/ir.rs:572`, the `Pagination` enum at `ir.rs:509`) still has **no reader
outside the loader** — `grep -rn pagination crates/connector-cli/src crates/connector-flux/src`
finds only a doc comment. Two providers declare it: twilio (`providers/twilio.toml:312`, `:393`)
and babelforce via patches (`providers/babelforce.toml:997`, `:1111`);
connector-surfaces.md's 2026-07-31 row also listed zendesk, which no longer declares one. The
member's cursor vocabulary states the same facts where a consumer can reach them, so the field is
**removed, not left unreachable** — the declarations migrate into datasource bindings or are
dropped with the removal reviewed in the same diff, and the `HashDomain` destructuring plus
`connectors.lock` make the schema change loud and versioned. That is
[C-514](../stories/C-514-retire-quirks-pagination.md).

## Stories

- [C-511](../stories/C-511-vendor-datasources-epic.md) — the epic.
- [C-512](../stories/C-512-datasources-ir-member.md) — the IR member: namespace, derived schema,
  per-verb bindings, `HashDomain`, load-time refusals.
- [C-513](../stories/C-513-publish-the-datasource-surface.md) — emission into manifest, public
  catalogue and embedded catalogue; never the module.
- [C-514](../stories/C-514-retire-quirks-pagination.md) — the `quirks.pagination` supersession.
- Amended in place rather than duplicated: [C-501](../stories/C-501-migrate-observability-plugins.md)
  and [C-502](../stories/C-502-migrate-data-and-secret-plugins.md) carry Decision 0006 rule 11's
  checkable no-deletion-without-mapped-replacement acceptance;
  [C-497](../stories/C-497-declare-runtime-operation-bindings.md) is cross-referenced as the owner
  of the stream/tail/lease terms — the one-shot cursor spelling is C-512's (see "The surface").

## Sequencing

Nothing here precedes the Milestone 1 first-run path. The surface lands with the Milestone 2
runtime-declaration work (C-497), the Exchange read seam with Milestones 2–3, streaming with
Milestone 3 — and rule 11 makes this surface a hard predecessor of the Milestone 4 migration waves:
no wave deletes a plugin whose manifest declares datasources until every declaration is mapped to a
proven connector datasource member or an explicit reviewed removal.
