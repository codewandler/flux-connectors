---
id: C-130
title: "The ivr service and its atomic operation inventory"
pillar: Spec
status: blocked
priority: 3
design: docs/designs/babelforce-ivr-atomics.md
epic: babelforce-ivr
areas: [providers, connector-spec]
note: "BLOCKED by its own inventory: the atomics have no wire identity. babelforce's parse_settings.go maps call-module names onto them, there is no endpoint per module, and the one Application CRUD resource is unmounted and already excluded as provisioning. Re-scope onto the six mounted /api/v3 endpoints"
---

# The ivr service and its atomic operation inventory

## Goal

Add `service = "ivr"` with `api_version = "2"` to `providers/babelforce.toml` and publish the atomic
call primitives as operations.

## Acceptance

- [ ] `[[services]]` gains `ivr` at `api_version = "2"`, alongside the existing surfaces. babelforce
      currently declares no services, so this story also places the existing `agent` and `call`
      operations into their own service — the loader refuses a file that declares any service while
      leaving an operation unassigned ([C-49](C-49-provider-services.md)).
      → **not done.** There is no `ivr` service to add (see Progress), and the `agent`/`call` split is
      not worth doing speculatively: naming a service renames the emitted artifacts and mints a new
      address, and an address once published is never reused. The item is also underspecified — it
      names two services for three resource groups, and says nothing about where `/api/v2/sessions/{id}`
      goes.
- [x] The **inventory is written down before any TOML** — a table mapping each `internal/modules/*`
      primitive to the operation(s) it becomes, with the vendor endpoint each one calls. Record it in
      the design doc. An inventory derived while editing TOML is an inventory nobody can review.
      → [`docs/designs/babelforce-ivr-atomics.md`](../designs/babelforce-ivr-atomics.md) §The
      inventory. No TOML was written, because the inventory said not to.
- [ ] Operations for the plain atomics: `audioplayer`, `read`, `switchnode`, `dial`, `recording`,
      `acd`. Each carries declared parameters, `risk` and `idempotency` chosen deliberately — `dial`
      places a real call and costs real money, and its risk must say so.
      → **blocked.** The atomics have no wire identity, there is no endpoint per module, and `dial`
      places no call. §What the inventory found records the five findings.
- [x] **No call module is published.** `simpleMenu`, `promptPlayer` and friends are compositions; a
      test or an explicit note records that they were considered and excluded, so a later contributor
      does not "helpfully" add them.
      → both: `crates/connector-flux/tests/babelforce_ivr.rs`
      (`no_babelforce_operation_is_named_after_an_ivr_call_module`, and
      `the_fence_would_catch_the_operation_a_contributor_is_tempted_to_add` proving the fence has
      teeth), plus the design doc's §Approach step 2.
- [x] `agentic` and `realtime` are **out of scope** — both appear to carry streaming or model
      semantics a request/response operation cannot express. Say so in Progress rather than guessing
      at them. → said in Progress, with what the source shows.
- [ ] Generated Flux parses, analyzes and is a fixed point of flux's own formatter — the standing
      per-provider gate.
      → vacuous: no new Flux. Asserted for babelforce's existing nine by
      `every_babelforce_operation_emits_an_analyzable_module_without_secret_material`, which is new
      coverage — babelforce had no per-provider contract test before.
- [x] No credential value anywhere, and no `example` on a `secret` field that looks like a real token.
      → nothing was added, and the new emission test asserts neither `babelforce.access_token` nor
      `BABELFORCE_ACCESS_TOKEN` reaches emitted Flux.
- [x] The build stays a fixed point and the full gate is green.
      → `build --provider babelforce`: *12 artifacts up to date; nothing written*. `diff --provider
      babelforce`: *12 artifacts up to date*. Full workspace test, clippy `-D warnings` and
      `fmt --check` all green, with **zero** red — not even the usual three, because no provider TOML
      changed.

## Notes

- **Read the source, not the API docs alone**: `~/babelforce/projects/ivr/ivr/internal/modules/`.
  `flows/*.yaml` shows how each module is actually configured in practice, which is better evidence of
  the real parameter set than any document.
- A connector **selects** the operations worth exposing (`vision.md`). If a primitive has twelve
  settings and two matter, expose two and say why.
- This story is deliberately operations-only. Events are [C-131](C-131-ivr-events.md); do not fold
  them in, because the event set has a naming problem that deserves its own review.
- Whole-catalogue artifacts are coordinator-owned as of C-104 — use a provider-scoped build as your
  gate and do not hand-edit a global index.

## Progress

**Blocked, by the inventory this story asked for first.** That instruction is what caught it: the
inventory was written from `~/babelforce/projects/ivr/ivr` before any TOML, and it contradicts the
premise. Full evidence in [the design doc](../designs/babelforce-ivr-atomics.md) §The inventory and
§What the inventory found; the five findings in one line each:

1. **The atomics have no wire identity.** `adapters/backend/settingsapi/parse_settings.go:12-170` is
   babelforce's own map, and it runs the *opposite* way to this epic's assumption: it accepts
   *call-module* names (`promptPlayer`, `audioPlayer`, `textToSpeech`, `inputReader`, `speechToText`,
   `simpleMenu`, `switchNode`, `transfer`, `recording`, `consumerQueue`, `acd`, `agentQueue`, `flow`)
   and maps them onto the atomics. The `v2.*` identifiers appear only in
   `internal/app/application.go:29-40` and in tests asserting that function's return value — never in
   any wire document. So the composition-vs-primitive refactoring the epic wants has already happened
   *inside* babelforce, behind its API; externally there is no `audioplayer` to address, only
   `promptPlayer`.
2. **No endpoint per atomic.** The whole module surface is one CRUD resource,
   `/api/v2/applications[/{id}]` (`adapters/api/apiv2/openapi.yaml:9-159`), with `module` as a body
   field. Six atomics would collapse onto one `POST`.
3. **That resource is already excluded.** `docs/designs/provider-operation-inventory.md:717` drops ~120
   manager operations and names *applications* first — account provisioning done in the babelforce UI.
   Reversing that for one group belongs to that document, not here.
4. **`dial` places no call and costs nothing.** Creating a `transfer`-module application writes
   configuration; the IVR engine places the call later, when a live call traverses it. Nothing in the
   vendor's public HTTP surface executes an atomic against a live call — the closest is
   `DELETE /internal/v1/calls/{id}`, on an unauthenticated internal listener (`adapters/api/api.go:147-188`).
   So the one `risk` value the acceptance mandates has no request to describe, and the other reading of
   the item — an operation that *invokes* the atomic — has no endpoint at all. Two readings, two
   different files.
5. **Nothing derivable to author from.** `apiv2` is generated but **never mounted** (`apiv2` appears in
   exactly three files, all its own); it models 2 of 15 wire module names; its `audioPlayer` half is
   copy-paste broken (`openapi.yaml:419-424`, `:447-452`, `:467-472` all declare
   `module: {pattern: inputReader}`); `switchNode`/`transfer`/`recording`/`acd` exist only as Go structs
   in an `internal/` package, mostly untagged, and the one persisted example in the tree
   (`internal/modules/acd/settings.go:52-79`) uses keys matching none of them. No production host is
   evidenced anywhere — the only absolute server URL is the **dev** host
   `https://ivr.api.latest.dev.babelforce.com`, the exact trap `providers/babelforce.toml:24-31`
   already records for the manager document. And `api_version = "2"` names the unmounted document; the
   mounted one is `v3`.

**`agentic` and `realtime` — out of scope, and the source agrees for a sharper reason than the story
guessed.** Both have `settingsapi` DTOs, so both are wire-reachable in principle. But `realtime` is an
RTVBP session (`internal/modules/realtime/rtvbp.go`, `rtvbp_adapter.go`) — a bidirectional streaming
protocol with its own auth (`realtime/auth.go`), which no request/response operation can express — and
`agentic` (`handler.go`, 621 lines) drives a model turn-by-turn over the live audio path. Neither is a
request that returns. They are also blocked behind findings 1-3 like every other module, so nothing
about them is decidable until this story is re-scoped.

**What landed:** the inventory, and the one conclusion of the epic that survives — the call-module
exclusion — fenced in `crates/connector-flux/tests/babelforce_ivr.rs` rather than left as prose.
babelforce had no per-provider contract test before; it has four tests now.

**What did not, deliberately:** the `agent`/`call` service split. It only pays for itself if `ivr`
arrives, it renames published artifacts permanently, and as written it names two services for three
resource groups.

**Re-scope, for whoever picks this up:** an `ivr` service over the six endpoints that *are* mounted
(`adapters/api/api.go:126-132`) — `GET /api/v3/calls/{id}`, `.../traces`, `.../traces/timeline`,
`GET /api/v3/tts/voices`, `POST /api/v3/tts`, `GET /api/v3/status`. `getCallTraces` is a genuinely
valuable runtime read and the TTS pair has complete schemas. Two questions first: **what host serves
the IVR API in production**, and may an operation declare a non-JSON response (`image/png`,
`audio/wav`)?
