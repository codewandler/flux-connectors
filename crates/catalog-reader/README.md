# codewandler-connector-catalog-reader

Dependency-free reader for the flux-connectors **catalog pack** — every connector's canonical
document, compiled into one embedded, versioned, digest-checked file.

The pack is built by [`flux-connectors build`](https://github.com/codewandler/flux-connectors)
from the reviewed per-provider documents (`catalog/<name>.catalog.json`). This crate embeds the
pack that matches its own release and serves it with **zero non-optional dependencies**: no JSON
parser, no compression codec, no hash crate — the container is offset-indexed UTF-8 and the
SHA-256 check is vendored.

```toml
[dependencies]
codewandler-connector-catalog-reader = "0.21"
```

The library name is `catalog_reader`:

```rust
// The embedded pack: the catalogue this crate was released with.
let zendesk = catalog_reader::provider("zendesk").expect("shipped provider");
let document: &str = zendesk.document(); // canonical JSON, byte for byte

let show = catalog_reader::operation("zendesk-ticket-show").expect("shipped operation");
assert_eq!(show.provider(), "zendesk");
let record: &str = show.record(); // the operation's own JSON object

// A newer catalogue than this crate was built with, from a file:
let pack = catalog_reader::Pack::load("catalog.pack")?;
assert!(pack.provider("zendesk").is_some());
# Ok::<(), catalog_reader::Error>(())
```

Records are canonical JSON **text** — bring whatever JSON parser you already have. `Pack::load`
refuses a wrong container version, a wrong document schema version, or a digest mismatch before
serving a single record, each by name.

For the typed `&'static` catalogue API (`providers()`, `operation()`, risk and idempotency enums,
embedded Flux), use
[`codewandler-connector-catalog`](https://crates.io/crates/codewandler-connector-catalog), which
re-exports this crate as `catalog::reader`.

License: MIT OR Apache-2.0.
