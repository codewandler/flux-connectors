---
id: C-405
title: "The catalogue publishes each connector's runtime"
pillar: Bridge
status: ready
priority: 5
note: "catalog::Provider has no runtime field, so a host cannot read how a connector executes — it has to derive it. That makes the multi-tenant refusal rule untestable against real catalogue data"
---

# The catalogue publishes each connector's runtime

## Goal

Publish how a connector executes — `http`, `socket`, `process`, `container`, `plugin`, `remote` — so a
host reads it rather than assuming it.

## Why

A host refuses a locally-executing runtime when it serves more than one tenant, because process,
container and raw-socket execution consume the host's own identity and network position. That refusal
is only mechanical if the runtime is a **declared fact** the host can read.

`catalog::Provider` carries no runtime field. Today every shipped connector is HTTP, so a consumer
derives `Http` and is right — and will keep being right until the first connector that is not, at
which point the derivation is silently wrong for exactly the case the refusal exists to catch.

Found while designing flux-exchange's invoke path, where the consequence is concrete: no shipped
connector exercises the refusal, so its test has to construct a fixture rather than use the catalogue.

## Acceptance

- [x] The IR carries a connector's runtime, defaulting to `http` so no provider definition changes.
- [x] It reaches **both** the manifest and `catalog.json`, and the Rust catalogue. A field that reaches
      the IR and stops there is the failure mode this repo has six of already.
- [x] **Failing-first test** — a provider declaring a non-`http` runtime round-trips to the published
      catalogue, failing before the field exists.
- [x] `cargo run -p connector-cli -- diff` stays clean, or every moved artifact is explained.
- [x] The vocabulary matches flux's, and drift between the two is checked rather than promised — a
      mirrored closed set that nothing verifies stops being closed at the seam.

## Progress
- `connector_spec::Runtime` is the closed set — `http`, `socket`, `process`, `container`, `plugin`,
  `remote` — with `Http` the default. `Connector::runtime` is a plain `Runtime`, not an `Option`:
  "unset" and "http" would be two spellings of one meaning.
- It reaches all three artifacts. `runtime = "http"` in every `.connector.toml`, `"runtime": "http"`
  on every `catalog.json` provider entry, and `runtime: crate::Runtime::Http` on every generated
  `catalog::Provider`. **Always stated, never elided** — a manifest that named the runtime only when
  it was unusual would leave a host inferring `http` from an absence, which is the derivation this
  story removes.
- An unrecognised word is refused at the parse, by `serde`, exactly as an unknown `Role` is:
  `tests/golden/unknown-runtime.error` pins the message, which quotes what was written and lists the
  six that exist. There is deliberately no arm in `validate` for it — a runtime that fell back to
  `http` on a typo is how a `process` connector ends up served by a multi-tenant host.
- Drift is checked at the two seams that exist inside this repository. The published JSON schema's
  `enum` and `default` are read from `Runtime::ALL` and `Runtime::default()` rather than hand-typed
  (`tests/runtime_vocabulary.rs`), and `catalog::Runtime` — a second copy, because the catalogue
  crate takes no dependencies — is held equal to the loader's set by
  `crates/connector-cli/tests/runtime_axis.rs`. Half of that second one is already structural:
  `connector_cli::catalog::runtime` matches exhaustively and names a `catalog::Runtime` variant per
  arm.
- **The flux seam itself is not machine-checkable today, and that is recorded rather than papered
  over.** The vocabulary's source is a design document in another repository, and the pinned flux
  crates publish no runtime type — 0.41's `flux-spec`, `-runtime`, `-core` and `-system` declare no
  such enum — so there is nothing to link. `tests/runtime_vocabulary.rs` says so in its module docs
  and names itself as where the comparison belongs if flux grows one.
- Regeneration moved 114 artifacts: 60 manifests, 53 generated catalogue modules and `catalog.json`.
  No `.flux` module moved, which is the expected shape — the module is `op` declarations, and how a
  connector executes is a decision a host makes before it loads one.

## Notes on what was left out
- **No `is_local()` predicate**, on either `Runtime`. Which runtimes a deployment admits is the
  host's judgment — flux-exchange's `Deployment::admits` — and this repository publishes vendor
  facts. The classification is stated in each variant's doc comment so a consumer writing that
  predicate has the reasoning, but the policy stays where it can see its own inputs.
- **No provider changed.** Every shipped connector is `http` and says so by default, so the 53
  provider definitions are byte-identical. The IR's canonical JSON and hash domain skip the field
  when it is `http`, so no `ir_sha256` moved either.

## Notes
- Related: the `docs/designs/ecosystem.md` runtime axis in the flux repository, and flux-exchange's
  `Deployment::admits`, which is the consumer this unblocks.
