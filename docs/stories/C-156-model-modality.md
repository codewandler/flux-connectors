---
id: C-156
title: "A model in the pool should say what it transforms — and openai-audio-speech is the first Text→Audio"
pillar: Spec
status: ready
priority: 4
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [connector-spec, providers, bridge]
note: "there is NO local TTS model in flux — flux-audio is sample-rate math, dependency-free, no model. But POST /v1/audio/speech IS request/response, so a TTS operation is connector-shaped and gives the modality axis a second value"
---

# A model in the pool should say what it transforms — and `openai-audio-speech` is the first Text→Audio

## The correction this story starts from

**There is no local TTS model in flux.** `crates/flux-audio` is, in its own words, *"sample-rate math
for realtime voice pipelines"* — PCM16 byte⇄sample conversion, a stateless `resample`, a streaming
`Resampler` that carries phase across packet boundaries, and a `Framer`. Its module doc is explicit:
*"Dependency-free and pure — no IO, no async, no allocation beyond the `Vec`s it returns."* It owns
sample **math** only.

What does exist near voice: the **OpenAI Realtime** provider (`flux-providers/src/realtime.rs`, behind
a `realtime` feature) — WebSocket, full-duplex, voice-to-voice, and *not* local. Nothing in either is a
TTS model this repository could describe.

So a story premised on "port flux's local TTS" would be premised on something that is not there.

## What is actually reachable, and it is better

**OpenAI's `POST /v1/audio/speech` is one request and one response.** That is exactly the shape this
repository compiles — unlike inference, which needs SSE streaming and a tool-calling loop
([C-123](C-123-decide-connector-inference.md) records why that stays with flux). A TTS operation is
connector-shaped, and it gives the modality axis a genuinely different second value instead of a
taxonomy with one entry.

## Goal

Give a model in [C-121](C-121-llm-catalogue-role.md)'s pool a **modality** — what it transforms — and
ship the first non-text one.

## Acceptance

- [ ] A closed **modality** vocabulary describing input→output, derived from what the catalogue can
      actually populate today rather than invented: at minimum `text→text` (chat/completions),
      `text→vector` (`openai-embeddings-create` already ships), and `text→audio` (this story).
- [ ] **It is a third axis, not a fourth spelling of the other two.** A *role*
      ([C-120](C-120-service-roles-declaration.md)) is a checkable capability contract; a *tag*
      ([C-153](C-153-service-tags.md)) classifies a service for filtering; a **modality** describes what
      a **model** transforms. Record the distinction where the other two are recorded, in
      [provider-roles.md](../designs/provider-roles.md), or the three will be conflated by the next
      reader.
- [ ] `openai-audio-speech` ships: `POST /v1/audio/speech`, with `model`, `input` and `voice`, declared
      `risk` and `idempotency` chosen deliberately.
- [ ] **The response is audio bytes, and that must be honest.** `http.request` returns one flat string
      (`HTTP {status}\n{headers}\n{body}`), so binary audio through the emitted path is at best a
      lossy string. Either declare that plainly in the operation's description, or refuse to ship the
      operation on the composite path and say why — do **not** publish an operation whose result a
      caller cannot use. See [C-127](C-127-truthful-output-typing.md); this is the same trap in a
      sharper form, because the payload is not even text.
- [ ] Modality reaches `catalog.json` and the model pool, and the explorer can filter by it.
- [ ] **Failing-first test:** a modality that does not describe a shipped operation's actual
      input/output is refused.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **Do the honesty check before the plumbing.** If binary audio cannot survive `http.request`'s flat
  string, then the *declaration* is still worth having — a host using the Tool pack can parse and return
  bytes ([C-115](C-115-request-delegation.md) is Rust) — but the composite `.flux` path cannot, and the
  catalogue must not imply otherwise. That asymmetry is exactly what C-127 exists to express, so these
  two stories should be read together.
- **Modality belongs to a model, not to a service.** `openai`'s one service publishes chat, embeddings
  and speech; they differ by *model*, not by surface. That is why this is not just another tag.
- `openai-models-list` is what makes the pool live where flux's static tables go stale — C-121 records
  that. A modality declared per operation is a start; a modality *discovered* per model from the
  vendor's own list response is the better end state, and probably a follow-up.
- If a local TTS model is genuinely wanted, that is a **flux** story and a technology adapter, not a
  connector: `vision.md` reserves protocol-rich local integrations for flux plugins. Say so rather than
  building a connector around a process on the same machine.

## Progress

- **The TTS in question has been found, and it is babelforce's, not flux's.** C-130's inventory of
  `~/babelforce/projects/ivr/ivr` enumerated what is actually *mounted* at `/api/v3` (behind
  `webAuth.EchoMiddleware()`), and two of the six endpoints are text-to-speech:

  | endpoint | operation | response |
  |---|---|---|
  | `GET /api/v3/tts/voices` | `listTTSVoices` | `TTSVoice[]`, filtered by `provider` |
  | `POST /api/v3/tts` | `TTSCreate` | **`audio/wav`** from `text` + `provider` + `voice` + `language` |

  So this repository's first `text→audio` operation is not OpenAI's `/v1/audio/speech` after all — it is
  babelforce's own, on a surface already in the fleet. And `listTTSVoices` is a *catalogue* of voices
  filtered by provider, which is the same discovery shape as `llm_catalogue`.

- **This changes the story's worked example, not its design.** Both are still one request and one
  response, so both are connector-shaped, and the honesty problem is unchanged and now sharper:
  `POST /api/v3/tts` answers **`audio/wav`**, and `http.request` returns one flat string. Binary audio
  through the composite path is not merely lossy — it is not usable at all. Either the operation
  declares that plainly, or it does not ship on that path. See
  [C-127](C-127-truthful-output-typing.md).

- **Sequencing:** these two endpoints belong with C-130's re-scope onto the six mounted `/api/v3`
  operations, not to a separate story. Take the modality *axis* here and let babelforce's re-scope ship
  the operations.
