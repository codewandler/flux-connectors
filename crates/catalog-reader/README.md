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
codewandler-connector-catalog-reader = "0.26"
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

## Fetching a newer catalogue than the crate embeds

Every `vX.Y.Z` release of
[flux-connectors](https://github.com/codewandler/flux-connectors/releases) carries the pack of that
release as two assets, attached by the release workflow rather than by hand:

```text
https://github.com/codewandler/flux-connectors/releases/download/vX.Y.Z/catalog.pack
https://github.com/codewandler/flux-connectors/releases/download/vX.Y.Z/catalog.pack.sha256
```

So a consumer with no Rust toolchain and no clone fetches the catalogue with `curl`, and one that
has this crate loads the fetched file. Verify out of band first:

```console
$ base=https://github.com/codewandler/flux-connectors/releases/download/vX.Y.Z
$ curl -fsSLO "$base/catalog.pack" -O "$base/catalog.pack.sha256"
$ sha256sum -c catalog.pack.sha256
catalog.pack: OK
```

`catalog.pack.sha256` is one line of `sha256sum` output over the whole asset, and the digest in it
is the value the release tag's `connectors.lock` `[pack]` row records. The workflow computes it from
the committed pack at the tag and fails rather than attaching a pack whose digest disagrees, so the
check above is against the number the repository published — not one the file asserts about itself.
Assets are the byte-identical committed pack; nothing is repacked or recompressed on the way out.
They exist for every release from `v0.22.0` on.

Then, in band, once:

```rust
let pack = catalog_reader::Pack::load("catalog.pack").expect("a verified pack");
let zendesk = pack.provider("zendesk").expect("shipped provider");
```

`Pack::load` verifies before it serves a single record, and every refusal is a named variant of
`Error`:

- `UnsupportedFormat { found }` — the pack declares a container format newer than this reader
  implements. Upgrade the reader; the file is not corrupt.
- `UnsupportedSchema { found }` — the pack's documents carry a schema version this reader does not
  serve. It fails closed rather than handing out records it cannot vouch for.
- `DigestMismatch { stated, computed }` — the pack's header digest is not the digest of its own
  content: truncation, corruption or a hand edit.
- `NotAPack`, `NotText`, `Malformed(_)`, `Io(_)` — not a pack, not UTF-8, structurally not a
  version-1 pack, or unreadable.

**Two digests, deliberately.** The out-of-band one above covers the asset as a whole and belongs to
the tag; the in-band one is a header line covering every byte after itself, which is why it is a
different value and not comparable to the first. The pair is the point: a client checks before it
parses, the reader checks before it serves, and both are checking bytes the same release produced.
Neither is an authentication boundary — an author who can rewrite the payload can rewrite the digest
line above it — so the out-of-band value is the one that lives outside the file, in the tag and its
lockfile.

For the typed `&'static` catalogue API (`providers()`, `operation()`, risk and idempotency enums,
embedded Flux), use
[`codewandler-connector-catalog`](https://crates.io/crates/codewandler-connector-catalog), which
re-exports this crate as `catalog::reader`.

License: MIT OR Apache-2.0.
