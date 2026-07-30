# Design: Connectors v1 — spec to Flux

**Status:** proposed · **Pillar:** Core · **Stories:** C-1 … C-16

## Why

Prove the whole thesis on two real providers, end to end against a live flux.

A provider TOML plus a vendored vendor spec must compile into a `.flux` module that flux loads as ops
and exposes as LLM tools, with credentials resolved by the host and never present in any artifact.

This epic exists to answer one question before the repo grows a long tail of providers — **is a
generated connector actually as good as a hand-written plugin?** If patching a real vendor spec turns
out to be harder than writing the Rust, we want to learn that on provider two, not provider twenty.

The mechanics are in [connector-pipeline.md](connector-pipeline.md); the one blocking change to
`../flux` is in [auth-seam.md](auth-seam.md).

## Approach

Five tranches, sequenced so the longest-lead-time item starts early:

1. **Foundation** (`C-1`) — the Cargo workspace, three crates, the gate, the flux-lang pin.
2. **Bridge** (`C-16`) — design the `$auth` seam and file the implementation stories on flux's board
   *immediately*. It ships in a different repository on a different cadence, so it blocks the finish,
   not the start. Everything else proceeds without it.
3. **Spec** (`C-2`–`C-7`) — the IR, the TOML front-end, OpenAPI ingest, auth extraction, the overlay
   layer, provenance and the lockfile. No network; pure functions from bytes to IR.
4. **Codegen** (`C-8`–`C-12`) — IR to `flux_lang::ast` to formatted `.flux`, plus the manifest, plus
   quirks as Flux control flow. `C-11` (every generated module parses *and analyzes*) is the
   load-bearing test of the entire repo.
5. **Build** (`C-13`–`C-15`) — the CLI: hermetic build from the vendored cache, drift detection,
   install, and the live end-to-end run.

### The two providers, and why both

They exercise different halves, which is why milestone 1 takes both rather than one:

- **anthropic** — spec-driven, raw-header auth (`x-api-key`). Its auth already works with today's
  `http.request`, so it proves ingest → IR → codegen → registered op with no external blocker. It is
  the first thing that can go green.
- **zendesk** — Basic auth, heavy patching, and a direct comparison against flux's existing
  `plugins/zendesk` (687 lines of Rust). It proves the overlay layer, forces the auth seam, and tests
  the plugin-replacement claim.

Note that a generated Anthropic connector is a pipeline test fixture and a convenience surface, not
flux's inference path — flux talks to model vendors through `flux-providers`.

## Alternatives considered

- **One provider first, then the other.** Lower risk per step, but Anthropic alone would validate
  only the easy half: no Basic auth, no serious patching, no plugin comparison. The interesting
  failure modes all live in Zendesk.
- **Start with Zendesk alone.** Strongest proof, but auth-gated from day one — nothing could run
  end-to-end until a flux release landed, leaving the pipeline unverified for the whole wait.
- **Defer the auth seam and ship a pre-composed-credential escape hatch.** Fastest path to green.
  Rejected on the grounds in [auth-seam.md](auth-seam.md); it would have shipped an operator-hostile
  credential story that we would then have had to migrate away from.

## Risks & open questions

- **Cross-repo dependency.** Milestone 1 cannot complete until the `$auth` change ships in flux.
  Mitigated by sequencing `C-16` second and keeping every other story independent of it.
- **The overlay layer is the real bet.** If patching a bad vendor spec is harder than hand-writing an
  integration, the thesis fails. Zendesk is the test.
- **Op naming is a public contract** and must stay stable across regeneration.
- **Response shaping** (context-budget blowout from whole API payloads) is knowingly deferred past
  this epic.

## Acceptance / done

`flux-connectors build && flux-connectors install`, then a `flux` session lists
`zendesk.ticket.show` and `anthropic.messages.create` among its ops and calls one successfully
against the live API — with no credential present in any provider TOML, generated `.flux` file, or
lockfile.
