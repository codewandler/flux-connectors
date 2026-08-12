//! **The catalog pack: one file compiled from the canonical documents** (C-537).
//!
//! The canonical per-provider documents (C-536) are the reviewed source; the pack is the
//! *distributed* form — a single versioned, digest-carrying container the dependency-free reader
//! crate embeds and hosts can load from a path. The container properties are fixed by
//! `docs/designs/catalog-artifact.md` §2: one file, versioned schema, embedded digest,
//! deterministic bytes, offset-indexed reads, no network and no filesystem walk at query time.
//!
//! What this file asserts is the *writer's* half: a full build derives exactly one pack from the
//! documents it planned, byte-deterministically, records it in `connectors.lock`, and a scoped run
//! leaves it alone (the whole-tree comparison in `catalog_index.rs` covers that half). The
//! *reader's* half — parsing, refusal on a wrong version or digest — lives with the reader crate,
//! `crates/catalog-reader`, which owns the format's consuming side.
//!
//! The header is parsed here by position, not by search: the payload is JSON text, so looking for
//! a line that "starts with" a keyword could match inside a document. A format this file cannot
//! parse positionally is a format the zero-dependency reader cannot parse either.

use std::path::{Path, PathBuf};

use crate::common;

use crate::common::Fixture;

/// The pack, relative to a workspace root: inside the reader crate, which embeds it.
const PACK: &str = "crates/catalog-reader/catalog.pack";

/// The pack's magic-and-format-version line. Bumping the format is a new first line, which is what
/// lets an old reader refuse a new pack by name instead of misreading it.
const MAGIC: &str = "flux-connectors-catalog-pack 1";

/// Run a command through the real parser and the real `run`, exactly as `main` does.
fn run(args: &[&str]) -> anyhow::Result<String> {
    let invocation = connector_cli::cli::parse(args.iter().map(|arg| arg.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("output is UTF-8"))
}

/// The repository root, for the committed-tree assertions.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// A fixture holding three providers, so "one pack for the whole catalogue" is asserted over a
/// set a single provider cannot stand in for.
fn three_providers(label: &str) -> Fixture {
    let fixture = Fixture::with_provider(label, "acme");
    for provider in ["beacon", "cinder"] {
        fixture.write_provider(provider, &common::definition(provider));
        fixture.write_spec(provider, "v1", "{\"openapi\":\"3.0.3\"}\n");
    }
    fixture
}

/// One `p <id> <start> <len>` row: a provider's document, as a span into the payload.
struct ProviderRow {
    id: String,
    start: usize,
    len: usize,
}

/// One `o <id> <provider> <service> <start> <len>` row: an operation record, as a span into the
/// payload slicing its own JSON object out of the owning document.
struct OperationRow {
    id: String,
    provider: String,
    service: String,
    start: usize,
    len: usize,
}

/// The pack, parsed the way the format is defined: line by line, by position.
struct ParsedPack {
    /// The `schema <n>` line's value — the document schema version the payload carries.
    schema: u32,
    /// The digest the header states, and the region it must cover: every byte after the digest
    /// line itself.
    digest: String,
    digested_region_start: usize,
    providers: Vec<ProviderRow>,
    operations: Vec<OperationRow>,
    /// Where the payload begins, as a byte offset into the file.
    payload_start: usize,
    /// The length the `payload <len>` line declares.
    payload_len: usize,
}

/// Parse the v1 container positionally, panicking with the offending line on any deviation —
/// in a test, a malformed pack *is* the failure.
fn parse_pack(text: &str) -> ParsedPack {
    let mut offset = 0usize;
    let mut next_line = |what: &str| -> (String, usize) {
        let rest = &text[offset..];
        let end = rest
            .find('\n')
            .unwrap_or_else(|| panic!("the pack ends before its {what} line"));
        let line = rest[..end].to_string();
        offset += end + 1;
        (line, offset)
    };

    let (magic, _) = next_line("magic");
    assert_eq!(
        magic, MAGIC,
        "the pack's first line is the magic + format version"
    );

    let (digest_line, digested_region_start) = next_line("digest");
    let digest = digest_line
        .strip_prefix("digest sha256 ")
        .unwrap_or_else(|| panic!("not a digest line: {digest_line}"))
        .to_string();
    assert_eq!(digest.len(), 64, "a lowercase-hex SHA-256: {digest}");

    let (schema_line, _) = next_line("schema");
    let schema: u32 = schema_line
        .strip_prefix("schema ")
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("not a schema line: {schema_line}"));

    let (providers_line, _) = next_line("providers");
    let provider_count: usize = providers_line
        .strip_prefix("providers ")
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("not a providers line: {providers_line}"));

    let (operations_line, _) = next_line("operations");
    let operation_count: usize = operations_line
        .strip_prefix("operations ")
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("not an operations line: {operations_line}"));

    let mut providers = Vec::new();
    for _ in 0..provider_count {
        let (row, _) = next_line("provider row");
        let fields: Vec<&str> = row.split(' ').collect();
        assert_eq!(
            fields.len(),
            4,
            "a provider row is `p <id> <start> <len>`: {row}"
        );
        assert_eq!(fields[0], "p", "a provider row starts with `p`: {row}");
        providers.push(ProviderRow {
            id: fields[1].to_string(),
            start: fields[2]
                .parse()
                .unwrap_or_else(|_| panic!("bad start: {row}")),
            len: fields[3]
                .parse()
                .unwrap_or_else(|_| panic!("bad len: {row}")),
        });
    }

    let mut operations = Vec::new();
    for _ in 0..operation_count {
        let (row, _) = next_line("operation row");
        let fields: Vec<&str> = row.split(' ').collect();
        assert_eq!(
            fields.len(),
            6,
            "an operation row is `o <id> <provider> <service> <start> <len>`: {row}"
        );
        assert_eq!(fields[0], "o", "an operation row starts with `o`: {row}");
        operations.push(OperationRow {
            id: fields[1].to_string(),
            provider: fields[2].to_string(),
            service: fields[3].to_string(),
            start: fields[4]
                .parse()
                .unwrap_or_else(|_| panic!("bad start: {row}")),
            len: fields[5]
                .parse()
                .unwrap_or_else(|_| panic!("bad len: {row}")),
        });
    }

    let (payload_line, payload_start) = next_line("payload");
    let payload_len: usize = payload_line
        .strip_prefix("payload ")
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("not a payload line: {payload_line}"));

    ParsedPack {
        schema,
        digest,
        digested_region_start,
        providers,
        operations,
        payload_start,
        payload_len,
    }
}

/// **The acceptance's first bullet, in one place**: a full build derives a single pack file from
/// the canonical documents, embedding its schema version and content digest.
///
/// Every claim below is checked against the *documents the same build wrote*, so the pack is held
/// to being a projection of them rather than a second derivation that could drift.
#[test]
fn a_full_build_derives_one_pack_from_the_canonical_documents() {
    let fixture = three_providers("catalog-pack-derives");
    run(&["build", "--root", fixture.root().to_str().unwrap()]).expect("the full build succeeds");

    assert!(
        fixture.exists(PACK),
        "a full build must derive {PACK} from the canonical documents"
    );
    let pack = fixture.read(PACK);
    let parsed = parse_pack(&pack);

    // The embedded digest is real: SHA-256 over every byte after the digest line, spelled the way
    // `connectors.lock` spells every hash in this repository.
    assert_eq!(
        parsed.digest,
        connector_spec::sha256_hex(&pack.as_bytes()[parsed.digested_region_start..]),
        "the embedded digest must cover everything after the digest line"
    );

    // The embedded schema version is the documents'.
    assert_eq!(
        parsed.schema,
        connector_cli::document::SCHEMA_VERSION,
        "the pack states the canonical documents' schema version"
    );

    // The payload is exactly as long as declared, and exactly the concatenated documents.
    let payload = &pack.as_bytes()[parsed.payload_start..];
    assert_eq!(
        payload.len(),
        parsed.payload_len,
        "the declared payload length is the real one"
    );

    // One provider row per provider, ordered by id, each slicing its own committed document out of
    // the payload byte for byte.
    let ids: Vec<&str> = parsed.providers.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        ["acme", "beacon", "cinder"],
        "one row per provider, ordered by id"
    );
    for row in &parsed.providers {
        let document = fixture.read(&format!("catalog/{}.catalog.json", row.id));
        let slice = &payload[row.start..row.start + row.len];
        assert_eq!(
            slice,
            document.as_bytes(),
            "provider `{}`'s span must slice its canonical document out of the payload",
            row.id
        );
    }

    // One operation row per operation, ordered by id, each slicing a JSON object that *is* that
    // operation's record in the owning document.
    let op_ids: Vec<&str> = parsed
        .operations
        .iter()
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(
        op_ids,
        ["acme-thing-get", "beacon-thing-get", "cinder-thing-get"],
        "one row per operation, ordered by id"
    );
    for row in &parsed.operations {
        assert_eq!(row.provider, row.id.split('-').next().unwrap());
        assert_eq!(row.service, "default");
        let slice = std::str::from_utf8(&payload[row.start..row.start + row.len])
            .expect("an operation span is UTF-8");
        let record: serde_json::Value = serde_json::from_str(slice)
            .unwrap_or_else(|error| panic!("operation `{}`'s span is not JSON: {error}", row.id));
        assert_eq!(
            record["id"], row.id,
            "operation `{}`'s span must slice its own record",
            row.id
        );

        // The span points into the owning document, not at a re-serialization beside it: the
        // record must be a byte-identical substring of that provider's document.
        let document = fixture.read(&format!("catalog/{}.catalog.json", row.provider));
        assert!(
            document.contains(slice),
            "operation `{}`'s record must be sliced out of `{}`'s document, not restated",
            row.id,
            row.provider
        );
    }
}

/// **Byte-determinism, asserted the strong way**: two independent trees with the same inputs
/// produce the identical file, and a rebuild over an unchanged tree rewrites nothing.
///
/// The coordinator's build at integration must reproduce the committed pack byte for byte, so
/// determinism is not a nicety — a pack that varies run to run would read as drift in every
/// integration.
#[test]
fn the_pack_is_byte_deterministic() {
    let first = three_providers("catalog-pack-deterministic-a");
    let second = three_providers("catalog-pack-deterministic-b");
    run(&["build", "--root", first.root().to_str().unwrap()]).expect("the first build succeeds");
    run(&["build", "--root", second.root().to_str().unwrap()]).expect("the second build succeeds");

    assert_eq!(
        first.read(PACK),
        second.read(PACK),
        "equal inputs must produce a byte-identical pack in independent trees"
    );

    let rebuilt =
        run(&["build", "--root", first.root().to_str().unwrap()]).expect("the rebuild succeeds");
    assert!(
        rebuilt.contains("nothing written"),
        "an unchanged pack must not be rewritten: {rebuilt}"
    );
}

/// **The lockfile records the pack as a whole-catalogue artifact.** A per-provider row cannot
/// carry it — the pack belongs to no provider — so it is a `[pack]` section of its own, holding
/// the same lowercase-hex SHA-256 spelling every other recorded hash uses.
#[test]
fn the_lockfile_records_the_pack_as_a_whole_catalogue_artifact() {
    let fixture = three_providers("catalog-pack-lockfile");
    run(&["build", "--root", fixture.root().to_str().unwrap()]).expect("the full build succeeds");

    let lockfile = connector_spec::Lockfile::parse(&fixture.read(connector_spec::LOCKFILE_NAME))
        .expect("the emitted lockfile parses");
    let pack = lockfile
        .pack()
        .expect("a full build records the pack in connectors.lock");
    assert_eq!(
        pack.path, PACK,
        "the lockfile keys the pack by repository-relative path"
    );
    assert_eq!(
        pack.schema_version,
        connector_cli::document::SCHEMA_VERSION,
        "the lockfile records the schema version the pack embeds"
    );
    assert_eq!(
        pack.sha256,
        connector_spec::sha256_hex(fixture.read(PACK).as_bytes()),
        "the recorded hash is the hash of the emitted file"
    );
}

/// A `--provider` run compiled a subset, so it can no more write the pack honestly than it can
/// write the index — it must leave the committed file untouched rather than truncating it.
///
/// `catalog_index.rs` asserts this over the whole tree; this is the same property stated where a
/// reader of the pack's tests will look for it.
#[test]
fn a_scoped_build_leaves_the_pack_byte_identical() {
    let fixture = three_providers("catalog-pack-scoped");
    let root = fixture.root().to_str().unwrap().to_string();
    run(&["build", "--root", &root]).expect("the full build succeeds");
    let before = fixture.read(PACK);

    run(&["build", "--provider", "acme", "--root", &root]).expect("the scoped build succeeds");
    assert_eq!(
        before,
        fixture.read(PACK),
        "`build --provider` must not rewrite the whole-catalogue pack"
    );
}

/// **The committed pack is current**: recompiling the shipped catalogue plans the exact bytes the
/// repository carries. The pack-family sibling of
/// `catalog_document.rs::the_committed_documents_are_a_fixed_point_of_a_build`, scoped to the one
/// new artifact so its failure names the pack rather than whichever whole-catalogue file went
/// stale first.
#[test]
fn the_committed_pack_is_byte_identical_to_a_fresh_compile() {
    let workspace = connector_cli::workspace::Workspace::new(repo_root());
    let plan =
        connector_cli::pipeline::plan(&workspace, None).expect("every shipped provider compiles");

    let pack = plan
        .artifacts
        .iter()
        .find(|artifact| workspace.display_path(&artifact.path) == Path::new(PACK))
        .expect("a full plan claims the pack");
    assert_eq!(
        pack.change,
        connector_cli::pipeline::Change::Unchanged,
        "the committed {PACK} is stale; run `cargo run -p connector-cli -- build`"
    );
}
