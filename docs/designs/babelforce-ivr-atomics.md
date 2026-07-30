# Design: babelforce IVR v2 — atomics, not call modules

**Status:** proposed, and **§Approach step 1 is now blocked too** — the inventory C-130 asked for was
written from the Go source and it contradicts the premise. Read **§The inventory** and **§What the
inventory found** before scoping anything. **§The mismatch** still blocks C-132 independently ·
**Pillar:** Spec · **Stories:** [C-129](../stories/C-129-babelforce-ivr-epic.md) … C-132

> Read in `~/babelforce/projects/ivr/ivr` at the working tree present on 2026-07-30. Paths below are
> from that checkout.

## Why

babelforce's IVR v2 has two layers, and the connector has to choose which one it exposes.

**The module layer** — `internal/modules/`: `acd`, `agentic`, `audioplayer`, `dial`, `flow`, `read`,
`realtime`, `recording`, `switchnode`. These are the primitives: play audio, read a digit, branch,
dial, record, queue.

**The flow layer** — `flows/*.yaml`, where a node names a *call module* and carries `settings`:

```yaml
id: simple_menu_menu
module: simpleMenu
settings:
  prompts: [{ key: prompt, text: "Press 1 for Sales or 2 for Support" }]
  menuItems:
    - key: {name: KEY_1}
      application: {id: simple_menu_sales}
    - key: {name: KEY_2}
      application: {id: simple_menu_support}
  flowEndApplication: {id: simple_menu_no_result}
  readTimeout: 5
```

`simpleMenu` is not a primitive. It is `audioplayer` + `read` + `switchnode` welded together, with a
timeout. That is the whole observation behind this epic: **the call modules are compositions of the
atomics**, so exposing them as connector operations would publish seventeen frozen combinations
instead of six composable parts.

So the proposal — expose the atomics, and rebuild the call modules as templates over them — is the
right instinct. One thing makes it harder than it looks.

## The mismatch, which decides everything else

**A flow YAML is already a graph.** `flowEndApplication: {id}` is an edge. `menuItems[].application`
is a conditional edge. That is the same shape as [flow-graph.md](flow-graph.md)'s `Graph`, and the
resemblance is close enough to be tempting.

**But those edges are `goto`s**, and [C-94](../stories/C-94-flow-graph.md)'s model refuses exactly
that. Its rules are: no cycles, control flow must nest, no edge crosses a region boundary. Those
rules are not stylistic — they exist because **Flux has no `goto`**, so a graph that cannot be
topologically ordered cannot be lowered.

An IVR menu that re-prompts on invalid input jumps *backwards*. That is a cycle, and it is not an
edge case — it is what a menu **is**. `prompt_player_loop.yaml` is in the shipped flow set.

**A babelforce IVR flow is a state machine. C-94's graph is a nesting expression tree.** They are
different computational models, and treating one as the other is how this epic fails.

### The consequence people will miss: two different runtimes

A Flux composite op executes **in flux**. An IVR flow executes **in babelforce's own IVR engine** —
that is what `internal/ivr` and `internal/router` are. They are not the same runtime, and a graph
lowered for one does not run on the other.

`vision.md` already settles which side this repo is on: *"This repo compiles; flux executes."* An IVR
flow is a third thing — compiled here, executed by **the vendor**. Nothing in the current model
describes that, and pretending `Graph` covers it would produce artifacts that lower cleanly and run
nowhere.

## The inventory

Written for [C-130](../stories/C-130-ivr-atomics-inventory.md), from the Go source rather than from any
API document, and written **before** any TOML — which is why the TOML was never written. All paths are
in `~/babelforce/projects/ivr/ivr` at the 2026-07-30 working tree.

### Where an atomic is actually addressable

An atomic is not a resource, an endpoint or a verb. It is the value of **one body field** — `module` on
an `Application` — and the only IVR-v2 endpoints that carry that field are a CRUD resource:

| endpoint | operation | mounted? |
|---|---|---|
| `GET /api/v2/applications` | `listApplications` | **no** |
| `POST /api/v2/applications` | `createApplication` | **no** |
| `GET /api/v2/applications/{id}` | `getApplication` | **no** |
| `PUT /api/v2/applications/{id}` | `updateApplication` | **no** |
| `DELETE /api/v2/applications/{id}` | `deleteApplication` | **no** |

Declared in `adapters/api/apiv2/openapi.yaml:9-159`, generated into
`adapters/api/apiv2/server.gen.go:559-563`, and **never registered on a server**: the only three files
in the repository that mention `apiv2` are its own `server.gen.go`, `generate.go` and `server.yaml`.
`adapters/api/api.go:126-142` mounts `apiv3` and `webhooksv1` and nothing else.

### The atomic → wire mapping, which runs the other way

`adapters/backend/settingsapi/parse_settings.go:12-170` is babelforce's own map from the stored
`module` string to a handler. It is the authority, and it is **many wire names onto one atomic** —
the wire vocabulary is the *call-module* names, and the `v2.*` identifiers exist only as Go constants
in `internal/app/application.go:29-40` and in tests asserting that function's return value.

| `internal/modules/*` primitive | internal id | wire `module` values the vendor accepts | operation(s) it would become | vendor endpoint | settings source |
|---|---|---|---|---|---|
| `audioplayer` | `v2.audioPlayer` | `promptPlayer`, `audioPlayer`, `textToSpeech` | none — see below | `POST`/`PUT /api/v2/applications[/{id}]` | `ApplicationAudioPlayerSettings` (**empty**, and the schema is broken); `settingsapi/audio_player.go`, `tts.go` |
| `read` | `v2.inputReader` | `inputReader`, `inputReader.v2`, `inputReaderV2`, `speechToText`, `simpleMenu` | none | same | `ApplicationInputReaderSettings` (the one complete schema); `settingsapi/inputreader.go`, `input_reader_v2.go`, `stt.go`, `simple_menu.go` |
| `switchnode` | `v2.switchNode` | `switchNode` | none | same | **no schema** — `settingsapi/switchnode.go` only |
| `dial` | `v2.dial` | `transfer` | none | same | **no schema** — `settingsapi/transfer.go` only |
| `recording` | `v2.recording` | `recording` | none | same | **no schema** — `settingsapi/voice_recording.go` only |
| `acd` | `v2.ACD`, `v2.AgentQueue` | `consumerQueue`, `acd`, `agentQueue` | none | same | **no schema** — `settingsapi/acd.go` only |
| `flow` | `v2.flow` | `flow` | none | same | **no schema**; a YAML dialplan, and §The mismatch applies to it directly |
| `agentic` | `agentic` | `agentic` | out of scope | same | **no schema** — `settingsapi/agentic.go` |
| `realtime` | `realtime` | `realtime` | out of scope | same | **no schema** — `settingsapi/realtime.go` |

### What *is* mounted, and is therefore describable

`adapters/api/apiv3/openapi.yaml`, served at `/api/v3` behind `webAuth.EchoMiddleware()`
(`adapters/api/api.go:126-132`):

| endpoint | operation | response |
|---|---|---|
| `GET /api/v3/status` | — | `"OK"` |
| `GET /api/v3/calls/{id}` | `getCall` | `Call` — `id`, `type`, `from`, `to` |
| `GET /api/v3/calls/{id}/traces` | `getCallTraces` | `TraceEntry[]` — the per-application execution trace of a call |
| `GET /api/v3/calls/{id}/traces/timeline` | `getCallTracesTimeline` | `image/png` |
| `GET /api/v3/tts/voices` | `listTTSVoices` | `TTSVoice[]`, filtered by `provider` |
| `POST /api/v3/tts` | `TTSCreate` | `audio/wav` from `text` + `provider` + `voice` + `language` |

`adapters/api/internalv1/openapi.yaml` adds `POST /transcribe`, `DELETE /calls/{id}` and
`DELETE /calls/{id}/queue`, mounted on a **separate internal listener** with no authentication
(`api.go:147-188`). Those are the only calls in the tree that act on a live call, and they are not a
public surface.

## What the inventory found

Five independent findings, each of which alone stops "publish the atomics as operations". They are
facts about the vendor, not preferences.

**1 · The atomics have no wire identity, and the call modules are the only names that exist.** This is
the epic's premise inverted. `ParseModuleSettings` accepts `promptPlayer`, `audioPlayer`,
`textToSpeech`, `inputReader`, `speechToText`, `simpleMenu`, `switchNode`, `transfer`, `recording`,
`consumerQueue`, `acd`, `agentQueue`, `flow`, `agentic`, `realtime` — and maps them *onto* the seven
atomics. So "expose the six atomics rather than the seventeen combinations" describes a refactoring
that has already happened **inside** babelforce, on the far side of its API. Externally there is
nothing named `audioplayer` to address; there is `promptPlayer`, and babelforce decides internally that
`promptPlayer` is an audio player. An operation named for an atomic would be a name this repository
invented for a vendor concept the vendor does not expose.

**2 · There is no endpoint per atomic — there is one CRUD resource with a discriminator.** Six atomics
collapse onto `POST /api/v2/applications`, differing only in a body field. The design's own sentence
*"each is one request or one event, with declared parameters and declared risk"* is not true of any of
them.

**3 · That resource is the configuration CRUD this repository already excluded.**
[provider-operation-inventory.md](provider-operation-inventory.md) §5.3 drops ~120 manager operations
and names **applications first**: *"Account provisioning, done in the babelforce UI by an admin. A flow
reads call state; it does not create a routing."* Publishing six application-creates reopens a decision
that belongs to that document. `DELETE /applications/bulk` is in the same table's bulk-destructive row.

**4 · `dial` neither places a call nor costs money — because nothing executes an atomic over HTTP.**
Creating an application whose module is `transfer` writes configuration; the call is placed later, by
the IVR engine, when a live call traverses that application. C-130 requires `dial`'s `risk` to record a
money effect, and under the only reading that has an endpoint there is no money effect to record. Under
the reading where the operation *invokes* the atomic on a live call, there is no endpoint at all — the
closest thing in the tree is `DELETE /internal/v1/calls/{id}`, on an unauthenticated internal listener.
Two readings, two different files; that is the ambiguity that stopped the story rather than a
preference between them.

**5 · Neither the schemas nor the host can be derived; only guessed.**

- The document covers **2 of the 15** accepted wire module names — `inputReader` and `audioPlayer`.
- Its `audioPlayer` half is copy-paste broken. `ApplicationAudioPlayer` (`:419-424`),
  `ApplicationAudioPlayerCreateRequest` (`:447-452`) and `ApplicationAudioPlayerUpdateRequest`
  (`:467-472`) all declare `module: {pattern: inputReader}`, and two of the three point `settings` at
  `ApplicationInputReaderSettings`. `ApplicationAudioPlayerSettings` (`:433`) adds no properties of its
  own at all.
- For `switchNode`, `transfer`, `recording` and `acd` the parameter set exists only as Go structs in an
  `internal/` package, and mostly without `json` tags — `dial.Settings` carries `CallerID`, `Target`,
  `Timeout`; `acd.Settings` carries `QueueId`, `DialTimeout`, `QueueExperience`. The one persisted
  example in the tree (`internal/modules/acd/settings.go:52-79`) uses `ringTime`, `record`, `to`,
  `queueExperience` — keys that match **none** of those field names, because `settingsapi` translates.
  Authoring a body from the structs would be a schema that looks derived and is invented, which is
  exactly what [C-126](../stories/C-126-response-schema-coverage.md) refuses.
- **No production host is evidenced anywhere.** The only absolute server URL in either IVR document is
  `https://ivr.api.latest.dev.babelforce.com/v{2,3}` — a **dev** host, and
  `providers/babelforce.toml:24-31` already records that precise trap for the manager document. A
  service owns its `base_url`, so guessing one decides where a connector's egress goes.
- C-130 specifies `api_version = "2"`, which names the unmounted document. The mounted one is `v3`.

### What C-130 should become

Split in two, and neither half is this story:

- **An `ivr` service over the six mounted `/api/v3` endpoints.** `getCallTraces` is a genuinely
  valuable runtime read — the per-application trace of what a call actually did — and `TTSCreate` and
  `listTTSVoices` are ordinary request/response operations with complete schemas. It needs
  `api_version = "v3"`, and it needs one question answered first: **what host serves the IVR API in
  production?** Note `getCallTracesTimeline` returns `image/png` and `TTSCreate` returns `audio/wav`;
  whether an operation may declare a non-JSON response is a separate open question.
- **A decision on whether IVR applications are provisioning.** If the answer is that a connector should
  be able to build an IVR flow, that reverses inventory §5.3 for one group and belongs there, with the
  vendor's `apiv2` document mounted and its `audioPlayer` schemas fixed first.

And the exclusion the epic was right about stands on its own, so it is fenced now rather than deferred:
`crates/connector-flux/tests/babelforce_ivr.rs` fails if any babelforce operation is ever named after a
call module.

## Approach

> **Step 1 below did not survive the inventory.** Read §What the inventory found. Steps 2 and 3 are
> unaffected — step 2 is *more* right than it was, for a reason it did not anticipate.

**1 · Expose the atomics as operations and events.** `provider=babelforce`, `service=ivr`,
`api_version=2` — the service level from [C-49](../stories/C-49-provider-services.md) is exactly the
right granularity, and babelforce already has `agent` and `call` surfaces that stay untouched.

The atomics are what a connector is for: each is one request or one event, with declared parameters
and declared risk. `audioplayer`, `read`, `switchnode`, `dial`, `recording`, `acd`.

**2 · Do not publish the call modules as operations.** `simpleMenu` and `promptPlayer` are
compositions; publishing them freezes a combination and hides its parts. This is the same rule
`vision.md` already states as a non-goal — *"Mechanically emitting all 400 endpoints of a large spec
produces an unusable tool catalog"* — applied one level up.

The inventory adds a second reason this design did not have: the call-module names are also the **only**
`module` values the vendor's API accepts. So "do not publish the call modules" and "publish the atomics"
are not two halves of one plan — together they exclude the entire module layer, which is why C-130
published no operation at all.

**3 · Events are the reverse direction, and one of them needs naming carefully.** "on invite" is
**not** the SIP INVITE of an inbound call. In this codebase `invite` is the ACD **inviting an agent**
to take a queued call — `internal/modules/acd/handler.go:290-297`, where `q.callAgent(inviteCtx,
agent)` runs per candidate agent. Both events are real and worth exposing; they must not share a
name. [C-58](../stories/C-58-inbound-events-epic.md)'s `EventDecl` already models this.

**4 · Templates are deferred behind a decision, not designed now.** Whether "template-like components
built from atomics" belong here depends on whether the graph gains an explicit state-machine profile
with bounded loops and vendor-side execution, or whether IVR composition simply stays in babelforce's
flow YAML. C-132 decides it. **Nothing in steps 1–3 depends on that answer**, which is why they go
first.

## On "we should probably just get rid of those"

Worth separating two claims, because only one of them is this repo's business:

- **In the connector: yes.** The call modules should not be operations. That is decided here.
- **In babelforce's backend: not this repo's call.** Whether `internal/modules/simpleMenu` should be
  deleted in favour of composed atomics is a change to a production telephony service with its own
  tests, flows in the field, and customers' numbers pointed at it. This design can say the atomics are
  sufficient to express it; it cannot say the migration is safe. If that is wanted, it is a story on
  babelforce's own backlog with a migration and a rollback, not a consequence of a connector epic.

## Out of scope

- **Editing anything under `~/babelforce`.** This repo describes; it does not refactor the vendor.
- **Lowering an IVR flow to Flux.** Blocked on C-132, and blocked *hard* on the goto/cycle mismatch.
- **`agentic` and `realtime` modules.** Both look like they carry streaming/model semantics that a
  request/response operation cannot express. Scope them only after the six plain atomics land.
