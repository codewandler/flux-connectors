# Design: the RFC Editor corpus — an authoritative public corpus, read offline from a snapshot

**Status:** proposed · **Pillar:** Connector · **Story:**
[C-524](../stories/C-524-rfc-editor-corpus-datasource.md) · **Epic:** `vendor-datasources` ·
**Authority:** `../flux-roadmap/decisions/0006-datasources-are-declared-read-surfaces.md` rules 2,
5–8, 10 and 12, and `0008-host-local-integrations-use-declared-local-capabilities.md` rule 1 ·
**Connector shape:** [vendor-datasource-declarations.md](vendor-datasource-declarations.md) ·
**Runtime placement:** [all-integrations-are-connectors.md](all-integrations-are-connectors.md)

> **Two kinds of claim, deliberately kept apart.** Every *repository* fact below was produced by a
> command in the session that wrote this document — 2026-08-12, at `428938cd` — and the command
> travels with it. Every fact about the *RFC Editor's own surfaces* was not: this repository reaches
> no network, nothing was fetched, and the story's own reconnaissance note is a timestamped claim
> rather than something re-measured here. Those are marked **[assumed]** inline and collected once
> under [Assumptions to confirm](#assumptions-to-confirm). Re-grep by symbol; line numbers move.

## Why

The RFC Series is the one corpus an agent doing protocol work needs constantly, cites by number, and
must not paraphrase. It is public, credential-free, immutable per record, and identical for every
tenant — which makes it the cleanest possible test of Decision 0006's family sentence, *operations
do; datasources know*.

It is also the case where the obvious implementation is wrong twice. Fetching `rfcNNNN.txt` on each
read pays network latency for bytes that cannot change, and searching the RFC Editor's web UI turns
a citable corpus into scraped markup with no identity. The story's answer — index the authoritative
corpus once, then serve bounded search and get from an atomic local snapshot — is right, and it
raises exactly the contract questions this document exists to fix before anyone writes code.

**Nothing here is implementable today, and that is a finding rather than a scheduling detail.** See
[Dependencies](#dependencies--what-is-blocked-and-what-is-not). The first acceptance item of C-524 is
this document; the rest wait.

## Scope — three things this is not

- **Not the catalogue datasource.** [connectors-datasource.md](connectors-datasource.md) is the
  datasource *about* connectors, compiled in and bound as an indexed backend. This is a *vendor*
  datasource: what an external corpus knows, declared here and executed by Exchange.
- **Not a new access mode, and not Flux's indexed backend.** The snapshot is an adapter-owned
  acceleration and availability artifact inside the Exchange runtime placement, exactly as C-524's
  own Notes require. Decision 0006 rule 8 forbids a local vendor adapter or local index *fallback*
  in Flux; it does not forbid the connector's own runtime from holding the bytes it reads. The
  distinction that keeps the rule intact: Exchange unavailable still means this datasource
  unavailable, because Exchange is what executes the read.
- **Not the RFC Editor's search UI.** The reviewed inputs are the mirror and index surfaces
  (§ [Source and provenance](#source-and-provenance--the-origins-are-pinned-and-there-is-no-second-one)).
  A scraped result page has no stable identity, no provenance and no license to be re-served.

## Source and provenance — the origins are pinned, and there is no second one

Three origins, declared by the connector, and nothing derives a fourth:

| origin | role | status |
|---|---|---|
| `https://www.rfc-editor.org/rfc-index.xml` **[assumed]** | the enumeration and per-RFC metadata | required |
| `https://www.rfc-editor.org/rfc/rfc<N>.txt` **[assumed]** | canonical per-RFC text and record identity | required |
| `rsync://rsync.rfc-editor.org/` module `rfcs-text-only` **[assumed]** | bulk mirror acceleration | optional, operator-enabled, never the default |

Four rules over that table, each of which has a way of being quietly dropped:

- **HTTPS is the baseline; rsync is an acceleration.** The baseline transport must work with no
  external binary on the host, because requiring `rsync` makes the connector unrunnable wherever it
  is absent and makes the test suite depend on it — which C-524's fixture acceptance explicitly
  forbids. The rsync module is reviewed here, and admitted as an operator-enabled fast path for the
  cold bootstrap only. It changes *how bytes arrive*, never *which record they are*: every file
  obtained by rsync is validated against the same per-input digest and the same size bound as the
  HTTPS path, and a record whose canonical URL is `https://www.rfc-editor.org/rfc/rfc8446.txt` says
  so regardless of which transport carried it.
- **A redirect off the pinned host is a refusal, not a follow.** TLS only; no plain HTTP; no
  redirect to another host, not even a subdomain. A pinned origin that follows redirects is an
  unpinned origin with extra steps.
- **An unavailable upstream never falls back to a third-party mirror.** Not to a CDN copy, not to a
  Git mirror of the corpus, not to a package registry that happens to vendor it, not "just for the
  bootstrap". Refresh fails, typed and observable, and the last-good snapshot keeps serving
  (§ [Snapshot lifecycle](#snapshot-lifecycle--the-swap-is-the-whole-design)). The reason is
  provenance and it is not decorative: a record that reports `rfc-editor.org` while its bytes came
  from elsewhere is a falsehood the corpus then repeats to every model that reads it, and the
  entire value of the RFC Series is that its text is the authoritative text.
- **The origin set is a declaration.** A caller — model input, Service Account, tenant setting —
  can never select, widen or substitute an origin. This is the same deputy argument
  [connectors-api.md](connectors-api.md) makes for the reference host: a caller who chooses the
  destination has chosen the effect.

## The entity contract — one identity, three ways in

Two entity kinds.

**`rfc`.** Id is the RFC number **as a string**: `"8446"`. Fields: canonical URL, title, authors,
publication date, stream, category, status, abstract, `obsoletes` / `obsoleted_by` / `updates` /
`updated_by` (each a list of `rfc` ids in the same string form), and normalized searchable body
text. Sub-series membership (STD/BCP/FYI) is a **field**, not a second entity kind, in v1 **[assumed
the index carries it]** — a sub-series is a label over RFCs, and minting a parallel identity for it
would give the same document two ids.

**`rfc_section`.** Id is `"<rfc>#<section>"` — `"8446#4.1.1"` — composite, derivable in both
directions, and carrying the parent id verbatim so a traversal never has to re-derive it. Fields:
heading text, the canonical URL of its RFC, the parent `rfc` id, and the section's own body range.

**RFC numbers stay strings at the wire boundary**, and the reason is worth stating because the
alternative looks tidier. An RFC number is an identifier, not a quantity: nothing sums it, and the
only arithmetic anyone does on it is ordering. `catalog.json` is a public consumer surface and a JSON
number is an IEEE double in most readers. The section id is a string whatever happens, so a numeric
`rfc` id would make the corpus's two identities differently typed for no gain. **The consequence to
state rather than discover: ordering is by the parsed integer, never by the string.** A corpus sorted
lexicographically puts RFC 1000 before RFC 999, and that is precisely the defect this choice invites.

**One identity, returned identically by search, get and relation traversal.** The id a search hit
carries is the id `get` accepts is the id an `obsoletes` list yields — byte-identical, with no
normalization step in between. This is a property to assert in a fixture, not a convention to
observe: an identity that agrees by habit disagrees on the first record whose relation was parsed by
a different code path.

**Two body forms, and only one is quotable.** The canonical bytes are stored unmodified so a citation
can quote them exactly. The searchable form is *derived* — page breaks and running headers/footers
removed, wrapped and hyphenated lines rejoined — and is never what a citation quotes. The derivation
is versioned as part of the snapshot schema version, because a normalizer change silently changes
what matches.

**Section identity is parser-derived, and the parser version is part of the contract.** The canonical
`.txt` rendering carries no machine-readable section markup **[assumed; RFC XML v3 exists for newer
RFCs only]**, so section ids come from a declared, versioned parser over the text. A section id is
therefore stable *for a given parser version*, and a parser bump is a snapshot rebuild with a new
schema version — never a silent re-parse that repoints existing ids at different text.

## Snapshot lifecycle — the swap is the whole design

**Two version numbers, not one.** `schema_version` covers the entity contract, the normalizer and the
section parser; `index_format_version` covers the full-text index alone. Conflating them would make
an index-format change a full re-download of the corpus, when it is rebuildable from bytes already on
disk.

**The manifest is the snapshot's identity**: snapshot id, origin, synchronized-at, both version
numbers, record counts, and a **per-input digest** for every fetched file and for the index document.
Results report snapshot identity, source provenance, synchronized-at and freshness — a caller that
cannot tell how old the corpus is will assume it is current.

**Readiness is earned, and an empty corpus is never the answer.** The datasource is not advertised
ready until a complete snapshot *and* its index exist and validate. With no valid snapshot the
datasource stays not-ready and returns a typed operator-facing remediation. Returning an empty result
set instead is the worst available failure: a model reads "no RFC matches" and concludes the RFC does
not exist.

**Cold bootstrap is an explicit, observable operation.** A declared prepare/refresh operation the
operator or the host runs — never the first model read. The corpus is large enough that hiding the
initial sync inside a read turns one question into a multi-minute stall with no progress and no
attribution **[assumed: corpus magnitude]**.

**Refresh is incremental after bootstrap.** Fetch the index, compare per-input digests, fetch only
what is new or changed. RFCs are immutable once published **[assumed]**, so the steady-state refresh
is the index plus the delta — and the design must not rely on that immutability for *correctness*,
only for cost: a changed digest re-fetches, whatever the reason.

**Refresh is single-flight per snapshot root**, held by a lease under that root. A second refresher
observes and reports in-progress; it never starts a second fetch against the same root. The precedent
is `connector-secrets`' one-writer lifetime lease (AGENTS.md, *Durable credential store contract*),
and the failure it prevents is the same one: two writers interleaving into a state neither of them
would have produced.

**Every byte is staged and validated before it counts.** Fetch into a staging directory under the
same root — same filesystem, so the swap is a rename rather than a copy — validate each file against
its declared digest, size bound and decodability, build the index over the staged bytes, then perform
**one atomic rename of the `current` pointer**. This follows the `FileStore` replacement discipline
recorded in
[portable-owner-only-secret-store.md](portable-owner-only-secret-store.md): render the complete
candidate beside the destination, flush, and use the operating system's atomic replacement primitive;
never open the destination with truncation.

**A reader resolves `current` once per call and holds it for the whole call.** Without that, a swap
mid-call splits one result across two corpora — a page of hits from snapshot N and a `get` from
snapshot N+1 — which is exactly the partially-refreshed corpus the acceptance forbids, arriving
through the back door.

**Retention is one previous snapshot.** The old snapshot stays until the new one is proven readable,
then goes. That bounds disk at roughly twice the corpus and gives the rollback path something to roll
back to.

## Search — and the verb the datasource surface does not have

**The finding first, because it changes where this contract lives.** Decision 0006 rule 2 gives *live*
mode `schema`/`list`/`get` with opaque cursors; `search` belongs to *indexed* mode.
[connectors-datasource.md](connectors-datasource.md):66 records the consequence in this repository's
own words — the live trait is *"`schema`/`list`/`get` with opaque cursors … **no search, no relation,
no batch-get**"*. And the planned `[[datasources]]` member has no search verb either:

```bash
grep -n -i 'search' docs/stories/C-512-datasources-ir-member.md \
  docs/stories/C-513-publish-the-datasource-surface.md \
  docs/designs/vendor-datasource-declarations.md   # → no output
```

So **C-524's search acceptance cannot be satisfied by the `[[datasources]]` member even after C-512
and C-513 land.** This design does not invent a seventh verb to close that gap — a search verb on a
connector datasource member is a Decision 0006 amendment and needs an owner outside this story.

**The resolution: ranked full-text search is a declared read operation, and the datasource member is
its neighbour.** That is not a workaround; it is what rule 6 already says, applied honestly. Every
datasource read *is* an admitted operation. The connector declares `rfc-search`, `rfc-get`,
`rfc-section-get` and `rfc-relation-list` as read-only operations with declared response schemas, and
the `[[datasources]]` member projects `list` and `get` over the two that fit the live verb set. An
agent reaching the corpus through the tenant Datasource binding gets list/get; an agent granted the
operation gets ranked search. If a search verb is later added to the member, `rfc-search` is what it
binds to and **nothing in the entity contract moves** — which is why fixing the entity contract now
is worth doing even though the surface is incomplete.

The contract itself, wherever it is served:

- **Searchable:** RFC number, title, author names, abstract, section headings, and normalized body
  text. Not titles-only, and not recent RFCs only — the corpus is complete or it is misleading.
- **Ranking is by ordered class, not by a summed score.** In order: (1) exact RFC-number match — the
  query parses as an RFC number and that RFC exists; (2) exact title match; (3) title or section
  heading term match; (4) abstract term match; (5) body term match. A tuned score has to be re-tuned,
  and a tie inside a score is broken by whatever order the index happened to yield. Within a class,
  order deterministically; the final tiebreak is ascending RFC number. C-140 already fixed
  determinism as an acceptance property for this repository's other search surface
  (`the_same_query_ranks_identically_across_runs`), and this one inherits it.
- **Filters restrict; they never reorder.** At least stream, category/status, and publication year
  (single year and range). A filter that also nudged rank would make "narrow the query" and "change
  the answer" the same gesture.
- **Every hit carries why it matched** — which field, on which term — per C-140's rule: a match a
  caller cannot explain is a match a caller cannot check, and a model handed an unexplained hit will
  invent the justification.
- **A query matching nothing returns empty with the query echoed**, never a best-effort nearest hit.
- **Paging is opaque and snapshot-scoped.** The cursor encodes the snapshot id. A cursor minted
  against snapshot N and presented after a swap to N+1 is **refused with a typed "snapshot moved,
  restart the query"** — never silently re-anchored, which would skip or duplicate records across a
  corpus that changed under the caller without telling them.
- **Bounds are declared, not implicit:** maximum hits per page, maximum snippet bytes per hit,
  maximum total response bytes. A result that would exceed a bound is truncated at a page boundary
  and says that it was.

## Cache ownership and placement

**The snapshot root is supplied by the host, never derived by the adapter.** This is deliberately
*unlike* `crates/connectors-api/src/secrets.rs:182-197`, which derives
`$XDG_DATA_HOME/connectors-api/credentials` and falls back to `$HOME/.local/share/…`. That derivation
is correct for a single-operator reference host and wrong here: Exchange is the execution placement,
it already owns per-tenant isolation, and an adapter reading `HOME` writes into whichever home the
worker happened to inherit — which is an ambient authority answer to a placement question.

**Never the workspace.** A build in this repository is offline and a fixed point, and C-429 makes
`build` *refuse* a committed file under an artifact root that no plan writes (AGENTS.md, *An artifact
no plan claims is refused, not deleted*). A corpus checked out under the repository would be caught
by that check on the day it appeared, which is the right outcome and the wrong way to discover the
rule.

**Corrupt state is quarantined, not deleted.** Corrupt, incompatible or truncated state moves aside
under a quarantine directory with its manifest intact, and the rebuild starts fresh. **The last
verified snapshot is never discarded to make room for a rebuild** — that is the whole point of
retaining one. If the corrupt snapshot *is* the last verified one, the datasource goes not-ready with
a typed remediation rather than serving a corpus it can only partly read; a half-readable corpus
answers "no such RFC" for the half it lost.

**Bodies are stored once.** A section is an offset range into its RFC's stored body, not a copy.
Storing section text separately doubles the corpus and creates a second place for the normalizer to
disagree with itself.

**Reads stay safe during rebuild, restart and concurrent access** — which the `current`-pointer
design gives for free: a rebuild writes only into staging, a restart re-resolves `current`, and
concurrent readers each hold a resolved snapshot for the duration of their own call.

## Safety envelope

- **Zero network on a read.** Search, get and relation traversal read the local snapshot and open no
  socket. Proven with the transport forced offline, per C-524's fixture list — a property asserted
  by a test rather than by the absence of a call site.
- **No caller-selected URL, path or mirror.** The origin set and the snapshot root are declarations.
  A caller cannot name a destination, a file, a mirror or a transport.
- **No fetched byte reaches a shell or an interpreter.** RFC text is plain text; nothing evaluates
  it, renders it as markup, or places it in an argv element. The optional rsync path is an
  **argv-only spawn with a fixed argument vector** — no shell, no caller-influenced element, and the
  module name is a literal, not a value assembled from anything fetched.
- **Effects are declared, and the vocabulary already exists.** `SemanticEffect`
  (`crates/connector-spec/src/ir.rs:211`) carries `read`, `network`, `write_file` and `write_db`
  among its ten tags, published into manifests and `catalog.json`. Reads declare `["read"]`; refresh
  declares `["network", "write_file"]`. **Note one live refusal that this connector is the first to
  strain:** `crates/connector-spec/src/provider.rs:5493` refuses `pure` with the reason *"every
  connector operation makes an external HTTP call"*. A zero-network read of a local snapshot is the
  first operation here for which that premise is false. The refusal stays correct — a read of mutable
  local state is not deterministic and side-effect free — but its stated reason must be re-derived
  when a rich runtime lands, or the next author reads it and concludes a local read cannot be
  declared at all.
- **No vendor credential.** The corpus is public and the connector declares no `[[auth]]`. It still
  declares an `authority` — `org.rfc-editor` is well-formed under
  `connector_address::validate_authority` (`crates/connector-address/src/address.rs:257`), which
  requires at least two labels of lowercase ASCII letters, digits and `-`. This is mandatory:
  `crates/connector-spec/tests/credential_paths.rs:613`
  (`every_shipped_provider_declares_an_authority_and_renders_a_credential_path`) asserts it over the
  whole `providers/` directory, and skips the credential-path half for a connector that declares no
  credential — which is precisely freshdesk's shape and will be this connector's.
- **Inputs are bounded before they are trusted.** Maximum bytes per fetched file, maximum files per
  refresh, maximum total snapshot bytes, and a refusal — never a truncation — for an oversized input
  or an unexpected content type. A truncated RFC that still parses is a corpus entry that quietly
  lies.

## Dependencies — what is blocked, and what is not

Verified in this session, with the commands.

**`[[datasources]]` does not exist, in any form.** Four independent checks, all negative:

```bash
grep -rn 'datasource' crates/connector-spec/src/ir.rs                        # → no output
grep -rln 'datasources' crates/                                              # → no output
grep -n 'datasource' crates/connector-spec/schema/provider-toml.schema.json  # → no output
grep -c '"datasources"' web/public/catalog.json                              # → 0
```

So no provider file can declare a datasource member today, and none could be published if it did.

**Its two owning stories are both unstarted.**

```bash
grep -n 'status:' docs/stories/C-512-datasources-ir-member.md \
                  docs/stories/C-513-publish-the-datasource-surface.md
# C-512 …:5:status: backlog
# C-513 …:5:status: backlog
```

**The runtime prerequisites are `ready`, not done** — the same command over C-497 and C-498 prints
`status: ready` for both. So there is no operation-to-runtime binding vocabulary and no artifact
identity/attestation contract; a corpus adapter has nothing to declare itself as.

**The search verb gap** is the material one and C-512/C-513 do not close it — see
[§ Search](#search--and-the-verb-the-datasource-surface-does-not-have) for the grep and the
resolution. Without a named owner for "where does ranked search over a vendor corpus live", C-524's
third acceptance item is contract-less even in a world where every listed prerequisite has landed.

**No local-storage capability kind exists to declare.** Decision 0008 rule 1 fixes a closed set —
*"at minimum a Unix-socket endpoint, a file-shaped secret, and a local/private network destination
class"* — and a writable snapshot root is none of the three. No story here owns extending it:
`grep -rln '0008' docs/stories/` names only `C-519` and the generated board, and C-519 *consumes*
the existing three rather than adding a fourth.

**The connector is single-tenant-only as the runtime axis currently stands, and that is a cost worth
escalating.** `Runtime` (`crates/connector-spec/src/ir.rs:965`) has six values, and its own doc
comment (`ir.rs:946-952`) states that *"a hosted deployment serving more than one tenant **refuses** a
locally-executing connector, mechanically, by reading this field"* — which covers `socket`,
`process`, `container` and `plugin`. `http` does not describe an adapter that holds a corpus on disk,
and `remote` moves the problem rather than solving it. There is no value for *connector-owned code
with declared local storage and no host identity*, and a public, credential-free corpus that is
byte-identical for every tenant is the clean case for one. That belongs to C-497's vocabulary work,
and it should hear about it before the vocabulary closes.

### What is buildable before C-512 and C-513

Honestly: **almost nothing, and nothing in `providers/` or `crates/`.**

- A provider TOML declaring `[[datasources]]` does not load — the key is unknown to the schema and
  the IR. A provider TOML declaring the *operations* without the member would ship a connector whose
  only reason to exist is a datasource it cannot declare, and it would occupy the `org.rfc-editor`
  authority under this repository's never-reused-address rule before the shape is settled. Do not.
- What *is* buildable now is this document: the source, entity, refresh, cache, indexing and search
  contract — C-524's first acceptance item, and the only one that does not need a surface.

The one genuine intermediate shape, stated with its cost: **the corpus adapter and its snapshot
format are not connector-IR-shaped at all.** They are a runtime artifact under C-498, and the
snapshot manifest, staging/validate/atomic-swap lifecycle, quarantine path, index format and ranking
could be specified, fixtured and implemented against fixtures with no IR surface whatsoever —
provided nothing is added to `providers/`, nothing claims a catalogue presence, and nothing in the
compiler crates gains a dependency on it (the `connector-cli` fence in AGENTS.md's ownership table is
not negotiable for this). The moment it needs a provider file it needs C-512. And the cost is real:
an adapter built before C-497 fixes the input/output/error/cancellation binding is an adapter whose
whole seam gets re-spelled, so building it early buys fixtures and buys a rewrite.

### Sequencing

1. **C-497** (runtime operation binding) and **C-498** (artifact identity and attestation) — both
   `ready`. C-497 additionally hears the runtime-axis gap above.
2. **C-512** and **C-513** — both `backlog`; the member and its publication into M, R and J.
3. **A named owner for ranked search over a vendor corpus.** Either a Decision 0006 amendment adding
   the verb, or an explicit ruling that it stays an operation as this design proposes. Until one
   exists, C-524's search acceptance has a contract (§ Search) but no home.
4. **A local-storage capability kind** under Decision 0008 rule 1.
5. **Then** C-524's implementation, in the order its own acceptance lists.

## Assumptions to confirm

Not verified here, because this repository reaches no network and nothing was fetched. Confirm each
against the RFC Editor before implementing anything that depends on it; none of these may be quoted
as established.

| # | assumption | what depends on it |
|---|---|---|
| A1 | `rsync.rfc-editor.org` exposes a module named `rfcs-text-only`, intended for keeping a local mirror current | the optional bulk path only; the HTTPS baseline is unaffected |
| A2 | `https://www.rfc-editor.org/rfc-index.xml` is the index URL, and it enumerates every RFC | enumeration completeness |
| A3 | the index carries title, authors, publication date, stream, category/status, abstract and obsoletes/updates relations in structured form | the entity contract's metadata half; if not, each field is parsed from the text and the parser is versioned like the section parser |
| A4 | canonical text is at `https://www.rfc-editor.org/rfc/rfc<N>.txt` with no zero-padding | record identity and canonical URL rendering |
| A5 | HTTPS responses support conditional requests (`ETag`/`Last-Modified`) | incremental refresh cost, not correctness — digests decide correctness |
| A6 | a published RFC's text is immutable | refresh cost, not correctness |
| A7 | corpus magnitude — record count and total bytes | bootstrap cost messaging, retention sizing, bounds |
| A8 | no bulk archive is available over HTTPS | whether the rsync path is the only bulk option |
| A9 | the canonical `.txt` rendering carries no machine-readable section markup, and RFC XML v3 covers newer RFCs only | section identity being parser-derived and parser-versioned |
| A10 | sub-series membership (STD/BCP/FYI) is expressed as a label over RFCs rather than as independent documents | sub-series being a field, not a second entity kind |
| A11 | the RFC Editor's published policy permits automated mirroring and re-serving of the text at the volumes a bootstrap implies, and states an expected rate | whether the bootstrap is admissible at all, and at what pacing |

A11 is the one that can invalidate the story rather than a detail of it. Confirm it first.

## Non-goals

- **The web search UI.** Named as excluded by the story and excluded here; a scraped result page has
  no stable identity and no provenance.
- **Errata.** Out of the first corpus per C-524's Notes. Admitting them later needs them to be
  separately identified RFC Editor records that do not modify the immutable RFC body and do not
  overstate their own status; a design that cannot promise both keeps them out.
- **Internet-Drafts.** A different corpus with different mutability and different citation rules.
- **Embedding or semantic search.** Ship deterministic lexical search and find out whether it is
  insufficient, exactly as [connectors-datasource.md](connectors-datasource.md) concluded for the
  catalogue — an embedding index is a second artifact to build, store, version and keep in step.
- **Any write.** The corpus is read-only in the strongest sense available: the upstream is not ours
  and the snapshot is derived. Nothing here mutates either.
- **A Flux-local index or vendor fallback.** Decision 0006 rule 8. Exchange unavailable means this
  datasource unavailable.
- **Presenting cached text as current implementation guidance.** A snapshot is as fresh as its last
  successful refresh, and results say so. Documentation must make citing the RFC Editor the obvious
  move and quoting a stale snapshot the awkward one.

## Stories

- [C-524](../stories/C-524-rfc-editor-corpus-datasource.md) — this corpus. Blocked; see
  [Dependencies](#dependencies--what-is-blocked-and-what-is-not).
- [C-512](../stories/C-512-datasources-ir-member.md) / [C-513](../stories/C-513-publish-the-datasource-surface.md)
  — the `[[datasources]]` member and its publication. Hard predecessors.
- [C-497](../stories/C-497-declare-runtime-operation-bindings.md) /
  [C-498](../stories/C-498-build-and-attest-runtime-artifacts.md) — the runtime binding and artifact
  contracts a corpus adapter declares itself under.
- [C-511](../stories/C-511-vendor-datasources-epic.md) — the owning epic;
  [vendor-datasource-declarations.md](vendor-datasource-declarations.md) is the surface design this
  one instantiates.
