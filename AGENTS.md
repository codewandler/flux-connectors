# AGENTS.md — operating contract for flux-connectors

This file is for coding agents and automation. Read it before changing the repository. It defines
how work is selected, which subsystem owns each concern, what must remain fail-closed, and what
evidence “done” requires. For a human introduction, use [README.md](README.md).

<!-- BEGIN track:agents -->
## Start here — mandatory workflow for every task

This repository uses the **track** framework. Every unit of work is a Markdown story in
`docs/stories/`; story frontmatter is the source of truth and the board in
[`docs/stories/README.md`](docs/stories/README.md) is generated from it.

1. **Orient.** Read the latest user request and run `git status --short --branch`. Uncommitted work
   is user-owned unless you created it during the current task; preserve unrelated changes.
2. **Select the work.** If the user named work, do that. Otherwise take the top `ready` story by
   priority (lower is higher). `/track:next` reports it; `/track:next <area>` filters by `areas`.
3. **Create a contract when needed.** New or unscoped work gets a story before implementation via
   `/track:story`. Read its `## Goal`, `## Acceptance`, and any linked `design:`. Acceptance defines
   done.
4. **Make the work visible.** Set the story to `in-progress` and run `/track:board`. For non-trivial
   design, write or update a record in `docs/designs/` before implementation.
5. **Implement and prove it.** Behavioral changes require a failing-first test. Keep generated
   output, docs, and code consistent; run the relevant checks while working.
6. **Close the work.** `/track:done <ID>` sets the story to `done`, adds the changelog entry, and
   regenerates the board. Confirm the acceptance checklist and the final diff before reporting.

Never hand-edit the generated region between `BEGIN track:board` and `END track:board`. After any
change to a story's `status`, `priority`, `title`, `epic`, or `note`, run `/track:board`. Optional
`areas: [subsystem]` tags affect queries only; they do not create board sections.
<!-- END track:agents -->

## Current project boundary

**Snapshot: v0.2.0.** `cargo run -p connector-cli -- build` compiles six providers and 38 curated
operations into 59 artifacts. The compiler, embedded Rust catalogue, JSON catalogue, and public
explorer work. **No generated provider can make a live API call yet.** Read
[Intentional gaps](#intentional-gaps) before changing code that appears broken.

flux-connectors compiles vendor API descriptions into Flux-Lang. A provider is described in
`providers/<name>.toml`; the build emits an installable Flux module, a capability manifest,
per-operation renderings, Rust catalogue tables, and public catalogue data. Flux—not TOML—is the
execution format.

The charter boundary comes first for any proposed provider:

- **Generated connectors belong here:** HTTP-based SaaS services such as Zendesk, Freshdesk,
  Salesforce, Intercom, OpenAI, Anthropic, and OpenRouter.
- **Hand-written technology adapters belong in `../flux/plugins`:** Docker, Kubernetes, SQL,
  Prometheus, Loki, Vault, Asterisk, and other stateful or protocol-rich systems.

If the target is a technology rather than a service, stop and place the work in flux unless an
accepted design explicitly changes this charter.

## Ownership boundaries

| Crate | Owns | Must never |
|---|---|---|
| `connector-spec` | IR, provider-TOML loading, validation, provenance, lockfile | Touch the network |
| `connector-flux` | Lowering the IR to `flux_lang` AST and formatting Flux | Emit Flux with string templates |
| `connector-cli` | Binary, orchestration, filesystem IO, and all future network IO | Reach the network during `build`, `diff`, or `check` |
| `connector-catalog` | Static provider/operation metadata and embedded Flux | Execute operations, touch the network/filesystem, or gain runtime dependencies |

`connector-spec` ingest accepts bytes so it remains fully unit-testable. Vendor network access, when
implemented, belongs only in `connector-cli`'s `fetch` path.

## Source and generated-file boundaries

Generated artifacts are committed and reviewed, but they are not edited by hand. Change their
source or emitter, then run:

```bash
cargo run -p connector-cli -- build
cargo run -p connector-cli -- diff
```

`diff` must finish with `59 artifacts up to date (6 providers checked)` for the current catalogue.
The artifact count may legitimately change when providers or operations change; do not encode it as
a permanent invariant.

| Generated path | Source of truth |
|---|---|
| `connectors/*.flux`, `connectors/*.connector.toml` | `providers/`, vendored `specs/`, compiler code |
| `crates/catalog/ops/<provider>/*.flux` | Emitted provider operations |
| `crates/catalog/src/generated/<provider>.rs` | Connector IR and catalogue emitter |
| `web/public/catalog.json` | Connector IR and public-catalogue emitter |
| `assets/readme-snippet-{light,dark}.svg` | `assets/readme-snippet.flux` and flux highlighter |

Two nearby files are intentionally hand-maintained:

- `assets/readme-snippet.flux` is the checked source for the README image; tests keep it identical to
  the operation shown in the generated Zendesk module.
- `crates/catalog/src/generated.rs` is the provider module index. A provider-scoped build cannot
  regenerate a complete global index, so tests keep this explicit list in sync.

The public VitePress site is a consumer surface, not a publication of repository internals. Public
pages may explain connectors, operation contracts, safety metadata, credentials, hosts, and current
availability. Do not publish or link internal designs, roadmap/story mechanics, crate architecture,
or agent instructions there. Those belong in the repository's `docs/` tree.

## Non-negotiable engineering rules

- **TOML is compiler input, never a runtime execution format.** Do not move behavior into config
  that a runtime reads directly.
- **Do not create a connector-specific DSL.** Interpolation, branching, retries, and error handling
  are Flux language concerns.
- **Emit through `flux_lang`, never string templates.** Build `flux_lang::ast` nodes and format with
  flux-lang's formatter.
- **Generated Flux must parse and analyze in CI.** This is the load-bearing compatibility gate.
- **Generation is explicit, committed, deterministic, and offline.** Never hide network access in a
  `build.rs` or in `build`, `diff`, or `check`.
- **No credential value enters provider TOML, generated Flux, a manifest, the public catalogue, or
  the lockfile.** Generated data carries credential references and environment-variable names only.
- **Refuse ambiguous or unsafe output.** A loud compile-time refusal is better than plausible but
  incorrect Flux.
- **Library errors use `thiserror`; the binary uses `anyhow`.** No `unwrap()` on fallible IO outside
  tests.

## Authentication contract

Authentication is modelled on three independent axes: **source × acquisition × placement**. Never
replace it with a flat enum of combined schemes; that becomes combinatorial and forces credential
assembly into generated code. See [docs/designs/unified-auth.md](docs/designs/unified-auth.md).

Generated Flux names a credential and nothing more. It must not add prefixes, base64-encode pairs,
refresh tokens, or perform session login. The host resolves the credential, performs effectful
acquisition such as OAuth2, applies the placement scheme, and registers values with its redactor.
Putting acquisition in Flux would expose raw tokens in model-visible symbols.

Flux's four existing `AuthScheme` variants are presets of the three-axis model. A connector using
only those presets must serialize exactly what flux already understands.

## Service contract

A provider is one vendor; a **service** is one of its API surfaces (`s3` and `bedrock-runtime` under
AWS). The service is the unit that is addressed, versioned, selected, emitted and installed. See
[docs/designs/provider-services.md](docs/designs/provider-services.md).

- **Services partition the operation set.** Every operation belongs to **exactly one** service; the
  per-service sets are pairwise disjoint and their union is every operation. This is what makes
  "install the whole `s3` service" a well-defined set, and it is asserted as a property
  (`crates/connector-spec/tests/service_partition.rs`). Do not replace it with a free-form `tags`
  field: a tag cannot partition an operation set, cannot carry a version, and cannot carry a host.
- **`default` is reserved, implicit, and elided.** An operation naming no service belongs to
  `default`; no `[[services]]` entry may declare it; and it is **never rendered** into an address
  (`com.freshdesk.api:v2`, not `com.freshdesk.api/default:v2`) or into a file name (`zendesk.flux`,
  not `zendesk-default.flux`). A provider that declares named services has no implicit `default` for
  an operation to fall into — omitting `service` there is a loud error.
- **A service owns its base URL and its API version**, with the connector's as defaults. Each emitted
  manifest carries its own service's `base_url` and its own operations, so a service's egress surface
  is never widened to the union of the provider's. C-10's `http_hosts` derives from that value.
- **No content field of a provider TOML influences an output path.** Paths derive from the discovered
  file stem, and the one content field that reaches a path — a service name, via
  `<provider>-<service>.flux` — is validated against the address grammar in the loader
  (`connector_spec::address::validate_service_name`). A name carrying `/` or `..` would let a provider
  file decide where a build writes, including outside the repository root. Validate any future field
  that reaches a path the same way, at the loader, before it reaches `Workspace`.
- **An address, once published, is not reused.** Renaming a service or an operation mints a new
  address and deprecates the old one; it never repoints an existing one. An `authority`, a service
  name and an `api_version` are checked against the grammar on load, so a rendered address always
  parses back to the value it was rendered from.

## Intentional gaps

These failures are recorded decisions. Do not “fix” one without reading its story and design.

- **No provider can make a live call.** `http.request` can replace a whole value with
  `{"$secret": "ENV"}`, but it cannot assemble a Bearer header or a Basic pair. The required
  `$auth` seam is designed in [docs/designs/auth-seam.md](docs/designs/auth-seam.md) and must land in
  flux.
- **Freshdesk declares no credential.** Its API key occupies the Basic username position, which the
  current model treats as non-secret config. Emitting it would bypass secret gating and redaction;
  the deliberate result is a fail-closed 401.
- **`zendesk-ticket-search` is non-functional.** Query values are not percent-encoded. Spaces are a
  misleading test because URL parsing can rescue them; `&`, `#`, and `+` corrupt the request, and
  `x&per_page=1` injects a parameter.
- **Some operations are refused during emission.** Examples include a nested body path without a
  `wire` field, a dotted operation id, and an ambiguous free-form body. Each refusal names its
  owning story.
- **`check`, `fetch`, and `install` are unimplemented.** They exit explicitly and point to C-14 or
  C-15. Do not turn them into partial, best-effort behavior.

## Validation

The full repository gate is:

```bash
cargo fmt --all
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Read `cargo test --workspace` correctly: it stops at the first failing test binary. A count of green
summaries does not prove the remaining binaries ran. As a diagnostic, this must print nothing:

```bash
cargo test --workspace 2>&1 | grep -E "FAILED|error: test failed|panicked at"
```

The public site has a separate Node 22+ gate:

```bash
cd web
npm ci
npm run build
npm test
```

For a truly docs-only change, narrower checks are acceptable. State exactly what ran. Changes to
README Flux examples must run `cargo test -p connector-cli --test readme_snippet`. Changes under
`web/` must run the site build and tests. Changes to generated public catalogue data or Rust emitters
are not docs-only and require the relevant Rust tests plus formatting and clippy.

## Relationship to flux

- flux-connectors depends on `codewandler-flux-lang` (library `flux_lang`) from crates.io, pinned in
  `[workspace.dependencies]`. Do not replace it with a git or `../flux` path dependency; those do not
  resolve in a fresh clone and couple the build to an unpublished tree.
- flux-connectors does not depend on the flux runtime. It compiles; flux executes.
- The critical runtime change is flux's `$auth` support for `http.request`. Its design is recorded
  here; implementation stories belong to flux's own board.
