# `ops/` — generated, do not edit

Every `.flux` file below this directory is written by `flux-connectors build` from
`providers/<name>.toml`, and every one of them is embedded into the `catalog` crate at compile time
by `src/generated/<provider>.rs`. Edit a provider definition and rebuild; an edit made here is
overwritten by the next build and fails
`crates/connector-cli/tests/catalog_artifacts.rs` in the meantime.

One directory per provider, one file per operation, named by the operation's Flux symbol.

**These are not what you install.** `connectors/<provider>.flux` is the module flux loads from
`~/.flux/flows`, and it declares every one of that provider's operations. The files here are the
same `op` declarations split one per file so the catalog can address them individually — additional,
not a substitution.
