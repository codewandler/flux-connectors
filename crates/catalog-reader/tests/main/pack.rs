//! The reader's contract, held against the pack this crate actually ships (C-537).
//!
//! Two kinds of test, deliberately mixed in one file. The embedded-pack tests are the consumer's
//! view: the catalogue this release was built with answers the four questions, and every record
//! it serves is the JSON its canonical document carries. The refusal tests are the *loader's*
//! contract — a wrong container version, schema version or digest is refused before any record is
//! served, each by name — asserted over synthetic packs whose digests are recomputed with the
//! `sha2` dev-dependency, so the vendored SHA-256 is never grading its own homework.

use catalog_reader::{Error, Pack};

/// The committed pack, by the path this crate embeds it from.
fn committed_pack_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("catalog.pack")
}

/// A synthetic version-1 pack around `body`, its digest computed independently of the crate under
/// test.
fn with_digest(body: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(body.as_bytes()) {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("flux-connectors-catalog-pack 1\ndigest sha256 {hex}\n{body}").into_bytes()
}

#[test]
fn the_embedded_pack_serves_the_shipped_catalogue() {
    let providers: Vec<&str> = catalog_reader::providers().map(|p| p.id()).collect();
    assert!(
        providers.len() > 50,
        "the shipped catalogue has tens of providers, found {}",
        providers.len()
    );
    let mut sorted = providers.clone();
    sorted.sort_unstable();
    assert_eq!(providers, sorted, "providers are served in id order");

    let zendesk = catalog_reader::provider("zendesk").expect("the shipped catalogue has zendesk");
    assert_eq!(zendesk.id(), "zendesk");
    assert!(
        zendesk.document().contains("\"connector\": \"zendesk\""),
        "a provider's document is its canonical JSON"
    );

    let show = catalog_reader::operation("zendesk-ticket-show")
        .expect("the shipped catalogue has zendesk-ticket-show");
    assert_eq!(show.provider(), "zendesk");
    assert_eq!(show.service(), "default");
    let record: serde_json::Value = serde_json::from_str(show.record()).expect("a record is JSON");
    assert_eq!(record["id"], "zendesk-ticket-show");
    assert!(
        show.document().contains(show.record()),
        "a record is a slice of its owning document, not a restatement"
    );

    let of_zendesk: Vec<_> = catalog_reader::operations_of("zendesk").collect();
    assert!(!of_zendesk.is_empty());
    assert!(of_zendesk.iter().all(|op| op.provider() == "zendesk"));

    // Absence is an answer, not an error.
    assert!(catalog_reader::provider("no-such-vendor").is_none());
    assert!(catalog_reader::operation("no-such-op").is_none());
    assert_eq!(catalog_reader::operations_of("no-such-vendor").count(), 0);
}

/// Every record in the pack, held to the standard the writer claims: the operation's span slices
/// a JSON value that is exactly the document's own element, and the index's provider/service
/// facts agree with the record.
#[test]
fn every_embedded_record_agrees_with_its_canonical_document() {
    let pack = catalog_reader::embedded();
    let mut operations_seen = 0usize;
    for provider in pack.providers() {
        let document: serde_json::Value =
            serde_json::from_str(provider.document()).expect("a canonical document is JSON");
        assert_eq!(
            document["connector"],
            provider.id(),
            "a provider's span serves its own document"
        );
        let declared = document["operations"]
            .as_array()
            .expect("a canonical document has an operations array");
        let served: Vec<_> = provider.operations().collect();
        assert_eq!(
            served.len(),
            declared.len(),
            "`{}` serves as many operations as its document declares",
            provider.id()
        );
        for operation in served {
            let record: serde_json::Value =
                serde_json::from_str(operation.record()).expect("a record is JSON");
            assert!(
                declared.contains(&record),
                "operation `{}`'s record is not an element of its document",
                operation.id()
            );
            assert_eq!(record["id"], operation.id());
            assert_eq!(record["service"], operation.service());
            operations_seen += 1;
        }
    }
    assert_eq!(
        operations_seen,
        pack.operations().len(),
        "the flat listing and the per-provider listing partition the same set"
    );
}

#[test]
fn load_serves_the_committed_pack() {
    let pack = Pack::load(committed_pack_path()).expect("the committed pack loads");
    assert_eq!(
        pack.digest(),
        catalog_reader::embedded().digest(),
        "the committed file and the embedded bytes are the same pack"
    );
    assert_eq!(pack.schema_version(), catalog_reader::SUPPORTED_SCHEMA);
    assert!(pack.provider("zendesk").is_some());
}

#[test]
fn a_tampered_payload_is_refused_before_any_record() {
    let mut bytes = std::fs::read(committed_pack_path()).expect("the committed pack is readable");
    let last = bytes.len() - 2;
    bytes[last] = bytes[last].wrapping_add(1);
    match Pack::from_bytes(bytes) {
        Err(Error::DigestMismatch { stated, computed }) => {
            assert_ne!(stated, computed);
        }
        other => panic!("a tampered payload must refuse with DigestMismatch, got {other:?}"),
    }
}

#[test]
fn a_newer_container_format_is_refused_by_name() {
    let text = std::fs::read_to_string(committed_pack_path()).expect("the committed pack");
    let newer = text.replacen(
        "flux-connectors-catalog-pack 1",
        "flux-connectors-catalog-pack 2",
        1,
    );
    match Pack::from_bytes(newer.into_bytes()) {
        Err(Error::UnsupportedFormat { found }) => assert_eq!(found, 2),
        other => panic!("a newer format must refuse with UnsupportedFormat, got {other:?}"),
    }
}

#[test]
fn something_that_is_not_a_pack_is_refused() {
    match Pack::from_bytes(b"{\"this\": \"is json, not a pack\"}\n".to_vec()) {
        Err(Error::NotAPack) => {}
        other => panic!("a non-pack must refuse with NotAPack, got {other:?}"),
    }
}

/// The schema line sits *inside* the digested region, so a schema this reader does not serve is a
/// verified, well-formed pack that is refused anyway — fail closed, by name.
#[test]
fn a_newer_schema_version_is_refused_by_name() {
    let bytes = with_digest("schema 2\nproviders 0\noperations 0\npayload 0\n");
    match Pack::from_bytes(bytes) {
        Err(Error::UnsupportedSchema { found }) => assert_eq!(found, 2),
        other => panic!("a newer schema must refuse with UnsupportedSchema, got {other:?}"),
    }
}

/// Additive growth — an unknown header line, an unknown index-row kind — must not break a
/// version-1 reader: the digest vouches for the bytes, and anything a reader must not ignore is a
/// format bump, not a new line.
#[test]
fn additive_growth_is_tolerated() {
    let payload = "{\"id\":\"acme\"}\n";
    let body = format!(
        "schema 1\nflavor experimental\nproviders 1\noperations 1\n\
         p acme 0 {len}\no acme-thing-get acme default 0 {len}\n\
         e acme-event acme 7 4\npayload {len}\n{payload}",
        len = payload.len()
    );
    let pack = Pack::from_bytes(with_digest(&body)).expect("unknown lines are additive, not fatal");
    assert_eq!(
        pack.provider("acme").expect("acme is served").document(),
        payload
    );
    assert_eq!(
        pack.operation("acme-thing-get")
            .expect("the operation is served")
            .record(),
        payload
    );
}

#[test]
fn a_span_outside_the_payload_is_refused() {
    let payload = "{\"id\":\"acme\"}\n";
    let body = format!(
        "schema 1\nproviders 1\noperations 0\np acme 0 {}\npayload {}\n{payload}",
        payload.len() + 7,
        payload.len()
    );
    match Pack::from_bytes(with_digest(&body)) {
        Err(Error::Malformed(what)) => assert!(what.contains("acme"), "names the row: {what}"),
        other => panic!("an out-of-bounds span must refuse as Malformed, got {other:?}"),
    }
}

#[test]
fn an_operation_naming_an_absent_provider_is_refused() {
    let payload = "{\"id\":\"acme\"}\n";
    let body = format!(
        "schema 1\nproviders 1\noperations 1\np acme 0 {len}\n\
         o ghost-op ghost default 0 {len}\npayload {len}\n{payload}",
        len = payload.len()
    );
    match Pack::from_bytes(with_digest(&body)) {
        Err(Error::Malformed(what)) => {
            assert!(what.contains("ghost"), "names the absent provider: {what}");
        }
        other => panic!("an orphan operation row must refuse as Malformed, got {other:?}"),
    }
}

/// A payload shorter than declared is caught even though the digest already passed — the digest
/// proves the bytes are the author's, the structure check proves the author's arithmetic.
#[test]
fn a_payload_length_disagreement_is_refused() {
    let payload = "{\"id\":\"acme\"}\n";
    let body = format!(
        "schema 1\nproviders 0\noperations 0\npayload {}\n{payload}",
        payload.len() + 3
    );
    match Pack::from_bytes(with_digest(&body)) {
        Err(Error::Malformed(what)) => {
            assert!(what.contains("payload"), "names the disagreement: {what}");
        }
        other => panic!("a length disagreement must refuse as Malformed, got {other:?}"),
    }
}

/// The vendored SHA-256 agrees with the `sha2` crate across lengths that cross every padding
/// boundary — the 55/56/63/64-byte edges where a hand-rolled implementation goes wrong.
///
/// The vendored implementation is private, so it is driven through the one public surface that
/// uses it: [`with_digest`] states the digest `sha2` computed, and a successful load *is* the
/// vendored implementation agreeing over the whole varying-length body.
#[test]
fn the_vendored_sha256_agrees_with_sha2_across_padding_boundaries() {
    for len in (0..=130).chain([1000, 4096, 100_000]) {
        let payload: String = (0..len).map(|i| char::from((i % 79 + 33) as u8)).collect();
        let body = format!(
            "schema 1\nproviders 0\noperations 0\npayload {}\n{payload}",
            payload.len()
        );
        Pack::from_bytes(with_digest(&body))
            .unwrap_or_else(|error| panic!("payload length {len}: the digests disagree: {error}"));
    }
}

/// Not part of the gate: a measurement, printed for the design record. Run with
/// `cargo test -p codewandler-connector-catalog-reader --release -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement for docs/designs/catalog-artifact.md, not an assertion"]
fn measure_read_costs() {
    let bytes = std::fs::read(committed_pack_path()).expect("the committed pack");
    let size = bytes.len();

    let started = std::time::Instant::now();
    let pack = Pack::from_bytes(bytes).expect("the committed pack verifies");
    let verified_in = started.elapsed();

    let started = std::time::Instant::now();
    let lookups = 10_000usize;
    let mut found = 0usize;
    for _ in 0..lookups {
        found += usize::from(pack.operation("zendesk-ticket-show").is_some());
        found += usize::from(pack.provider("stripe").is_some());
    }
    let looked_up_in = started.elapsed();

    println!(
        "pack: {size} bytes; load+verify: {verified_in:?}; {} lookups: {looked_up_in:?} ({found} hits)",
        lookups * 2
    );
}
