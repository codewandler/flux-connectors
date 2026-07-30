# AGENTS.md — operating contract for agents in flux-connectors

For **coding agents and automation**. Read this before making any change. When in doubt, this file
and the docs it links are the tie-breaker.

---

## What this repo is

flux-connectors compiles **vendor API specs into Flux-Lang**. A provider is described once in
`providers/<name>.toml`; the build emits `<name>.flux` (typed `op` declarations) and
`<name>.connector.toml` (the capability manifest). [flux](../flux) loads the module from
`~/.flux/flows` and every `op` becomes a first-class operation and an LLM tool.

Why: [docs/vision.md](docs/vision.md) · pipeline:
[docs/designs/connector-pipeline.md](docs/designs/connector-pipeline.md) · status:
[docs/roadmap.md](docs/roadmap.md).

---

## The charter boundary — what belongs here

**Connectors are paid SaaS services.** Zendesk, Freshdesk, Salesforce, Intercom, OpenAI, Anthropic,
OpenRouter. They are HTTP + auth + quirks, and they are generated.

**Technology adapters stay in flux as plugins.** docker, kubernetes, sql, prometheus, loki, vault,
asterisk. They are stateful and protocol-rich; hand-written Rust earns its keep there.

If a proposed connector wraps a *technology* rather than a *service*, it belongs in `../flux/plugins`,
not here. This boundary is the first question to ask about any new provider.

---

## Non-negotiable conventions

- **TOML is never the execution format.** It is input to a compiler. The artifact that runs is Flux.
  Any proposal that moves behavior into config a runtime reads directly is wrong — that is precisely
  the mistake this repo exists to correct (see [docs/vision.md](docs/vision.md)).
- **No homegrown DSL.** Interpolation, branching, and error handling are expressed in Flux, which
  already has a parser, an analyzer, and editor tooling. Never invent a second little language.
- **Emit Flux through `flux_lang`, never through string templates.** `connector-flux` builds real
  `flux_lang::ast` nodes and formats them with flux-lang's own formatter, so unparseable or
  non-canonical output is structurally impossible.
- **Every generated module must parse *and analyze*** against flux-lang in CI. This is the
  load-bearing test; without it invalid Flux can be committed.
- **Generated artifacts are committed and reviewed.** Generation is an explicit CLI run, never a
  network-touching `build.rs`. Builds are hermetic and offline from the vendored spec cache under
  `specs/`.
- **No credential ever enters a provider TOML, a generated `.flux` file, or the lockfile.** The
  generated call carries an auth *reference*; flux resolves it, applies the scheme, and registers the
  value with its redactor.
- **`connector-spec` performs no network IO.** Ingest takes bytes so it stays fully unit-testable.
  The network lives in `connector-cli` alone.
- **Errors** use `thiserror` in library crates; the `flux-connectors` binary uses `anyhow`. No
  `unwrap()` in non-test code on fallible IO.

---

## Dev loop — run before calling a change done

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings   # must be clean
cargo fmt --all                                          # then commit the result
```

Docs-only changes may use a narrower check — say explicitly in the final report what was and was not
run.

---

## Relationship to ../flux

- flux-connectors **depends on** `codewandler-flux-lang` (lib `flux_lang`) as a git dependency pinned
  to a flux tag. Bumping that pin is a deliberate, reviewed change.
- flux-connectors **does not** depend on the flux runtime. It compiles; flux executes.
- One change is required **in flux** and is on the critical path: the `$auth` marker for
  `http.request`. It is designed here in [docs/designs/auth-seam.md](docs/designs/auth-seam.md); the
  implementation stories are filed on flux's own board.

<!-- BEGIN track:agents -->
## Start here (every session) — track backlog

This project tracks work with the **track** framework: every unit of work is a markdown story in
`docs/stories/`, and the board (`docs/stories/README.md`) is generated from story frontmatter.

1. **Orient** — read the latest user request, then run `git status --short --branch`. Treat
   uncommitted changes as user-owned unless you made them.
2. **What to work on** — if the user named work, do that. Otherwise open the
   [board](docs/stories/README.md) and take the top `ready` story by `priority` (lower = higher).
   `/track:next` reports it; `/track:next <area>` filters by optional story `areas`.
3. **The contract** — read the story's `## Goal` and `## Acceptance`; Acceptance defines "done". Read
   any linked `design:`.
4. **Do the work** — set the story `in-progress`; non-trivial design goes in `docs/designs/` first;
   implement; satisfy Acceptance with a **failing-first test**; keep the project's gate green.
5. **On done** — `/track:done <ID>`: set `status: done`, add a CHANGELOG entry, regenerate the board.
6. **New or unscoped work?** Create a story first (`/track:story`) so the next agent inherits the
   context.

The board's status lists are generated — after any change to a story's `status`/`priority`/`title`/
`epic`, run `/track:board`. Use optional `areas: [subsystem]` tags for query-only subsystem selection
without changing board rows. Story frontmatter is the single source of truth.
<!-- END track:agents -->
