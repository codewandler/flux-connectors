# flux-connectors

Compiles vendor API specs into **Flux-Lang**.

A provider is described once in `providers/<name>.toml` — a pointer at a vendored vendor OpenAPI
document plus the patches it needs. The build emits two committed, reviewable artifacts:

- `<name>.flux` — typed `op` declarations, built as real `flux_lang` AST nodes and formatted by
  flux-lang's own formatter, never by string templates.
- `<name>.connector.toml` — the capability manifest.

[flux](https://github.com/codewandler/flux) loads the module from `~/.flux/flows`, and every `op`
becomes a first-class operation and an LLM tool. No credential ever enters a provider TOML, a
generated `.flux` file, or the lockfile: the generated call carries an auth *reference* that flux
resolves at run time.

## Layout

| Crate                    | Role                                                                       |
| ------------------------ | -------------------------------------------------------------------------- |
| `crates/connector-spec`  | The connector IR and its front-ends (provider TOML, OpenAPI). No network IO. |
| `crates/connector-flux`  | Emits Flux-Lang modules from the IR via `flux_lang`'s AST and formatter.     |
| `crates/connector-cli`   | The `flux-connectors` binary: fetch, check, build, diff, install.            |

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

## Docs

Everything else lives in [`docs/`](docs/):

- [docs/vision.md](docs/vision.md) — why this repo exists.
- [docs/roadmap.md](docs/roadmap.md) — what is shipping and in what order.
- [docs/designs/](docs/designs/) — design docs, starting with
  [connector-pipeline.md](docs/designs/connector-pipeline.md).
- [docs/stories/README.md](docs/stories/README.md) — the status board.
- [AGENTS.md](AGENTS.md) — the operating contract, including the charter boundary that decides
  whether something is a connector here or a plugin in flux.

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
