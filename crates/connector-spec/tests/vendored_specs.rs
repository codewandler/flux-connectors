//! The vendored babelforce documents carry no credential and no internal marker, and say where they
//! came from.
//!
//! `providers/babelforce.toml` spent five paragraphs explaining why the one authoritative description
//! of this API was *not* in the repository: the upstream document embeds a credential-shaped example
//! block for a real test account, and this repository is public. C-415 vendors it anyway, scrubbed —
//! and a scrub is only worth the paper it is written on if something fails when it stops holding.
//!
//! That is what this file is. Every claim here is checked against the committed bytes rather than
//! against the script that produced them, because the two can disagree: someone edits a vendored
//! document by hand, or re-runs the script against a fresh pull that carries a secret the discovery
//! rule does not recognise. A test that re-derived the answer from the script would agree with the
//! script in both cases.
//!
//! # The four gates, and why each is not redundant with the others
//!
//! - [`no_credential_shaped_example_value_survives`] is **shape-based and forward-looking**. It knows
//!   no secrets; it refuses any hex-and-dash value of sixteen characters or more under a
//!   credential-named key. A future pull that introduces a *new* token fails here, which is the only
//!   one of these gates that can catch something nobody has seen yet.
//! - [`the_known_credential_literals_can_never_reappear`] is **exact and backward-looking**. It reads
//!   the SHA-256 of each scrubbed literal from the provenance file and refuses that literal's return
//!   anywhere, under any key, in any document. The digests are safe to publish and the literals are
//!   not, which is the whole reason the denylist is spelled in digests. This gate is what catches the
//!   case the shape gate structurally cannot: in these documents the `accessId` value is **reused as
//!   a plain `id:`** three lines above itself, and a key-scoped rule would have left it there.
//! - [`the_declarations_survive_the_scrub`] is the **counterweight**. Every gate above is satisfied by
//!   deleting the documents, and a scrub that removed the `accessId`/`accessToken` declarations would
//!   be a silent regression rather than a safety win: `providers/babelforce.toml` excludes the
//!   deprecated `X-Auth-Access-*` pair as an *overlay* decision, on the explicit condition that ingest
//!   keeps seeing it so drift-check keeps reporting on it. Values are scrubbed; declarations are not.
//! - [`no_internal_marker_survives_in_a_vendored_document`] and
//!   [`every_url_in_a_vendored_document_points_at_a_public_host`] are the leak gates, one by name and
//!   one by structure. The named list mirrors an internal file and can only refuse what somebody
//!   thought of; the host allowlist refuses an internal host nobody thought of, which is the failure
//!   mode a copied list has.
//!
//! # What is *not* here
//!
//! No test asserts an operation count or a byte size. Those move on every upstream pull for reasons
//! that are not defects, and a gate that goes red on a legitimate re-vendor is a gate people learn to
//! re-baseline without reading.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use connector_spec::{sha256_hex, SpecSource};

/// The five documents, by upstream name. A literal rather than a directory read, because the claim
/// "these five and no others" is exactly what [`no_pull_configuration_is_vendored`] needs to make:
/// deriving the set from the directory would make that test agree with whatever is in it.
const DOCUMENTS: [&str; 5] = [
    "manager",
    "task-automation",
    "task-schedule",
    "user",
    "auth",
];

/// Keys whose inline scalar value is a credential in these documents.
const CREDENTIAL_KEYS: [&str; 3] = ["accessId", "accessToken", "token"];

/// Hosts a vendored document may name.
///
/// Short and closed on purpose. Every internal marker worth the name is a hostname, so an allowlist
/// is the one leak check that refuses a host this repository has never heard of — which a copied
/// denylist, by construction, cannot.
const PUBLIC_HOSTS: [&str; 10] = [
    "services.babelforce.com",
    "www.babelforce.com",
    // Referenced by the documents' own prose and licence blocks.
    "jwt.io",
    "www.mit.edu",
    "en.wikipedia.org",
    "secure.wikimedia.org",
    // Third-party OAuth endpoints named in examples.
    "www.googleapis.com",
    "oauth2.googleapis.com",
    // Placeholder hosts in example payloads.
    "my.test.url",
    "assets.my-company.org",
];

/// Internal markers, mirroring `manager-sdk/scripts/leak-markers.regex` — the authority, which stays
/// internal, so this is a copy and is documented as one.
///
/// Two deliberate differences from upstream. The upstream regex names two AWS account ids; writing
/// them into a public repository to prove they are absent would be self-defeating, and they occur only
/// inside an ECR image URI, which `.dkr.ecr` and `amazonaws.com` already refuse. And the upstream
/// regex spells word boundaries that plain substring matching cannot; substring matching is the
/// *stricter* of the two, so the difference cannot open a hole.
const INTERNAL_MARKERS: [&str; 10] = [
    "gitlab",
    "nexus",
    ".dkr.ecr",
    "amazonaws.com",
    "latest.dev",
    "rc.dev",
    "preproduction",
    "npm-internal",
    "kubectl",
    "sbf/",
];

/// The repository root, derived from this crate's manifest directory so the test is independent of
/// the working directory a runner happens to use.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// `specs/babelforce`, the vendored cache this file is about.
fn vendored_dir() -> PathBuf {
    repo_root().join("specs").join("babelforce")
}

/// Every vendored document as `(file name, contents)`, sorted by name.
///
/// An empty directory is a failure rather than a vacuous pass: every gate below is a loop, and a
/// missing `specs/babelforce/` would satisfy all of them at once while checking nothing.
fn vendored() -> Vec<(String, String)> {
    let dir = vendored_dir();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
        panic!(
            "cannot read {} — the babelforce documents are not vendored: {error}",
            dir.display()
        )
    });

    let mut documents: Vec<(String, String)> = entries
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.is_file())
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a UTF-8 file name")
                .to_owned();
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            (name, text)
        })
        .collect();
    documents.sort();

    assert!(
        !documents.is_empty(),
        "{} holds no documents, so every gate in this file would pass vacuously",
        dir.display()
    );
    documents
}

/// The provenance file, parsed.
fn provenance() -> toml::Table {
    let path = repo_root().join("specs").join("babelforce.provenance.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    text.parse::<toml::Table>()
        .unwrap_or_else(|error| panic!("{} is not valid TOML: {error}", path.display()))
}

/// The `[[spec]]` entries, in file order.
fn provenance_specs() -> Vec<toml::Table> {
    provenance()
        .get("spec")
        .and_then(toml::Value::as_array)
        .expect("the provenance file declares `[[spec]]` entries")
        .iter()
        .map(|entry| {
            entry
                .as_table()
                .expect("a `[[spec]]` entry is a table")
                .clone()
        })
        .collect()
}

/// `key: value` for a line that carries an inline scalar, with surrounding quotes stripped from the
/// value. `None` for a blank line, a comment, a sequence item, or a key whose value is on the
/// following lines — which is precisely what a schema *declaration* looks like.
fn inline_scalar(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
        return None;
    }
    let (key, value) = trimmed.split_once(':')?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some((key.trim(), value.trim_matches(['\'', '"'])))
}

/// Whether a scalar carries credential-grade entropy: hex digits and dashes, long enough not to be an
/// enum value or a short identifier, and holding at least one hex digit that is not zero.
///
/// The last clause is what makes a *scrubbed* literal pass. The scrub zeroes every hex digit and keeps
/// the dashes and the length, so `00000000-0000-0000-0000-000000000000` still says "a UUID belongs
/// here" while carrying nothing — and this predicate and that transform are exact complements, which
/// is why neither needs to know what the other replaced.
fn is_credential_shaped(value: &str) -> bool {
    value.len() >= 16
        && value.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        && value.chars().any(|c| c.is_ascii_hexdigit() && c != '0')
}

/// Every maximal hex-and-dash run of sixteen characters or more in `text`.
///
/// A run is only taken when neither neighbour is alphanumeric, so a hex-looking slice of a longer
/// word is not mistaken for a standalone literal. `.` and `/` count as boundaries deliberately: a
/// literal embedded in a path or a dotted name is exactly the placement a value-scoped scrub is
/// supposed to reach, so it must be visible to the gate that checks the scrub reached it.
fn hex_runs(text: &str) -> BTreeSet<String> {
    fn word(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    let chars: Vec<char> = text.chars().collect();
    let mut runs = BTreeSet::new();
    let mut index = 0;
    while index < chars.len() {
        if !(chars[index].is_ascii_hexdigit() || chars[index] == '-') {
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() && (chars[index].is_ascii_hexdigit() || chars[index] == '-') {
            index += 1;
        }
        let before_is_word = start > 0 && word(chars[start - 1]);
        let after_is_word = index < chars.len() && word(chars[index]);
        if index - start >= 16 && !before_is_word && !after_is_word {
            runs.insert(chars[start..index].iter().collect::<String>());
        }
    }
    runs
}

// ---------------------------------------------------------------------------------------------
// What is vendored
// ---------------------------------------------------------------------------------------------

/// The five documents are here, each named for the date it was pulled.
///
/// The date is in the name because `info.version` cannot carry the identity: three of the five declare
/// the string `0.0.0-dev`, so a version-named file would collide with the next pull of a document that
/// had visibly changed.
#[test]
fn the_five_babelforce_documents_are_vendored() {
    let documents = vendored();
    let names: Vec<&str> = documents.iter().map(|(name, _)| name.as_str()).collect();

    for document in DOCUMENTS {
        let prefix = format!("{document}-");
        let matches: Vec<&str> = names
            .iter()
            .copied()
            .filter(|name| {
                // The suffix after the prefix must look like a date, so that `task-schedule-…` is
                // not counted as a `task-…` document by prefix alone.
                name.strip_prefix(&prefix)
                    .and_then(|rest| rest.strip_suffix(".openapi.yaml"))
                    .is_some_and(|date| date.len() == 10 && date.matches('-').count() == 2)
            })
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "expected exactly one vendored `{document}` document named `{document}-<YYYY-MM-DD>.openapi.yaml`, found {matches:?} among {names:?}"
        );
    }

    assert_eq!(
        documents.len(),
        DOCUMENTS.len(),
        "{} holds {names:?}, which is not the five vendored documents and nothing else",
        vendored_dir().display()
    );

    for (name, text) in &documents {
        assert!(
            text.starts_with("openapi: 3.0.3\n"),
            "{name} does not open as an OpenAPI 3.0.3 document"
        );
    }
}

/// The pulled bytes are vendored here; the pull *configuration* is not.
///
/// `sources.json` holds the GitLab host and the project ids, and `scripts/pull.sh` holds how to reach
/// them. Those are the two files in the upstream `specs/` directory that must never cross into a
/// public repository, and "we did not copy them" is a claim worth a test rather than a memory —
/// re-vendoring is a script that copies a directory, and the next hand at it may widen the copy.
#[test]
fn no_pull_configuration_is_vendored() {
    let dir = vendored_dir();
    let entries: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| {
            entry
                .expect("a readable directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    let stray: Vec<&String> = entries
        .iter()
        .filter(|name| !name.ends_with(".openapi.yaml"))
        .collect();
    assert!(
        stray.is_empty(),
        "{} holds {stray:?}. Only the vendored OpenAPI documents belong here: the upstream directory \
         also carries `sources.json` and `scripts/pull.sh`, which name an internal GitLab host and \
         its project ids. Provenance goes in `specs/babelforce.provenance.toml`, beside this \
         directory rather than in it.",
        dir.display()
    );
}

// ---------------------------------------------------------------------------------------------
// Credentials
// ---------------------------------------------------------------------------------------------

/// No value that looks like a credential survives under a credential-named key.
///
/// Shape-based, so it holds against a document nobody has read yet: a fresh pull that introduces a new
/// token under `accessToken` fails here without anyone having to add it to a list.
#[test]
fn no_credential_shaped_example_value_survives() {
    let mut hits: Vec<String> = Vec::new();

    for (name, text) in vendored() {
        for (number, line) in text.lines().enumerate() {
            let Some((key, value)) = inline_scalar(line) else {
                continue;
            };
            if !CREDENTIAL_KEYS.contains(&key) {
                continue;
            }
            if is_credential_shaped(value) {
                hits.push(format!("{name}:{} — `{key}`", number + 1));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "a credential-shaped example value is present in a vendored document. This repository is \
         public. Re-run `scripts/vendor-babelforce-specs.sh <path-to-manager-sdk/specs>`; if that \
         leaves it in place, the discovery rule in the script has gone stale against upstream and \
         needs widening before these bytes are committed.\n  {}",
        hits.join("\n  ")
    );
}

/// The literals that *were* scrubbed cannot come back, under any key, in any document.
///
/// The denylist is the set of SHA-256 digests the scrub recorded in the provenance file. A digest is
/// publishable and its preimage is not, which is the only reason an exact gate can exist here at all.
///
/// This is the gate that covers the reuse case, and the reuse case is not hypothetical: in these
/// documents the `accessId` value is also the `customer.id` of the same example account, three lines
/// above. A rule scoped to credential-named keys would have scrubbed one and left the other.
#[test]
fn the_known_credential_literals_can_never_reappear() {
    let denied: BTreeMap<String, String> = provenance()
        .get("redaction")
        .and_then(toml::Value::as_array)
        .expect("the provenance file records the scrub as `[[redaction]]` entries")
        .iter()
        .map(|entry| {
            let entry = entry
                .as_table()
                .expect("a `[[redaction]]` entry is a table");
            let digest = entry
                .get("sha256")
                .and_then(toml::Value::as_str)
                .expect("a redaction records the digest of what it replaced")
                .to_owned();
            let replacement = entry
                .get("replaced_with")
                .and_then(toml::Value::as_str)
                .expect("a redaction records what it put there instead")
                .to_owned();
            (digest, replacement)
        })
        .collect();

    assert!(
        !denied.is_empty(),
        "the provenance file records no redaction, so this gate would pass vacuously. The scrub \
         found nothing to scrub, which means the discovery rule has gone stale — not that the \
         documents are clean."
    );

    let mut hits: Vec<String> = Vec::new();
    for (name, text) in vendored() {
        for run in hex_runs(&text) {
            let digest = sha256_hex(run.as_bytes());
            if let Some(replacement) = denied.get(&digest) {
                hits.push(format!(
                    "{name} — a literal digesting to {digest}, which the scrub replaced with `{replacement}`"
                ));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "a credential literal that was scrubbed out has reappeared in a vendored document:\n  {}",
        hits.join("\n  ")
    );
}

/// The scrub takes values and leaves declarations.
///
/// This is the gate that stops "make the leak tests green" from being satisfied by deletion, and it is
/// load-bearing for a reason `providers/babelforce.toml` states at length: the deprecated
/// `X-Auth-Access-*` pair is excluded from the connector as an **overlay** decision, "and ingest must
/// keep *seeing* the pair — otherwise drift-check (C-14) stops reporting on it".
#[test]
fn the_declarations_survive_the_scrub() {
    let documents = vendored();

    // Every document declares its security schemes, and none of them lost the block to the scrub.
    for (name, text) in &documents {
        assert!(
            text.lines().any(|line| line.trim() == "securitySchemes:"),
            "{name} declares no `securitySchemes`; the scrub is removing declarations, not values"
        );
    }

    // The two documents that describe the account payload still declare its credential fields — as
    // schema properties, and as `required` members.
    let mut declaring = 0;
    for (name, text) in &documents {
        let declares_property = |key: &str| {
            text.lines()
                .any(|line| line.trim() == format!("{key}:") && line.starts_with(' '))
        };
        let requires = |key: &str| text.lines().any(|line| line.trim() == format!("- {key}"));

        if declares_property("accessId") {
            declaring += 1;
            assert!(
                declares_property("accessToken"),
                "{name} declares `accessId` but not `accessToken`; they travel as a pair"
            );
            assert!(
                requires("accessId") && requires("accessToken"),
                "{name} no longer lists `accessId`/`accessToken` as required; the scrub reached the \
                 schema, not only the example"
            );
        }
    }
    assert_eq!(
        declaring, 2,
        "expected the `manager` and `user` documents to declare the `accessId`/`accessToken` schema \
         properties. If upstream removed them, that is a finding to record — not something to relax \
         here, because this assertion is what proves the scrub is value-scoped."
    );
}

// ---------------------------------------------------------------------------------------------
// Leaks
// ---------------------------------------------------------------------------------------------

/// No internal marker appears in a vendored document.
#[test]
fn no_internal_marker_survives_in_a_vendored_document() {
    let mut hits: Vec<String> = Vec::new();

    for (name, text) in vendored() {
        let haystack = text.to_ascii_lowercase();
        for marker in INTERNAL_MARKERS {
            for (number, line) in haystack.lines().enumerate() {
                if line.contains(marker) {
                    hits.push(format!("{name}:{} — `{marker}`", number + 1));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "an internal marker appears in a vendored document. This repository is public, and these \
         markers are what `manager-sdk/scripts/leak-markers.regex` exists to keep out of one:\n  {}",
        hits.join("\n  ")
    );
}

/// Every URL in a vendored document points at a host on the allowlist.
///
/// The structural half of the leak gate. A denylist copied from another repository refuses what its
/// author thought of; this refuses everything else, which is where an internal host that nobody
/// anticipated would actually show up.
#[test]
fn every_url_in_a_vendored_document_points_at_a_public_host() {
    let mut hits: Vec<String> = Vec::new();

    for (name, text) in vendored() {
        for (number, line) in text.lines().enumerate() {
            for scheme in ["https://", "http://"] {
                let mut rest = line;
                while let Some(at) = rest.find(scheme) {
                    let after = &rest[at + scheme.len()..];
                    let host: String = after
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
                        .collect();
                    if !host.is_empty() && !PUBLIC_HOSTS.contains(&host.as_str()) {
                        hits.push(format!("{name}:{} — `{host}`", number + 1));
                    }
                    rest = after;
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "a vendored document names a host that is not on the public allowlist. Either it is an \
         internal host, which must not be here at all, or it is a legitimate new public one, which \
         belongs in `PUBLIC_HOSTS` with a note saying what it is:\n  {}",
        hits.join("\n  ")
    );
}

// ---------------------------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------------------------

/// Every vendored document is provenanced, and its recorded hash is the hash of its bytes.
///
/// `info.version` is `0.0.0-dev` on three of the five, so the hash is the identity. A recorded hash
/// that nothing recomputes is a comment.
#[test]
fn provenance_records_every_vendored_document_and_its_hash_matches() {
    let root = repo_root();
    let specs = provenance_specs();
    let vendored = vendored();

    assert_eq!(
        provenance()
            .get("version")
            .and_then(toml::Value::as_integer),
        Some(1),
        "the provenance file declares no format version"
    );
    assert_eq!(
        specs.len(),
        vendored.len(),
        "the provenance file records {} documents and {} are vendored",
        specs.len(),
        vendored.len()
    );

    let mut recorded: BTreeSet<String> = BTreeSet::new();
    for entry in &specs {
        let path = entry
            .get("path")
            .and_then(toml::Value::as_str)
            .expect("a `[[spec]]` entry records the path of what it describes");
        let bytes = std::fs::read(root.join(path))
            .unwrap_or_else(|error| panic!("the provenance file names {path}, which {error}"));

        assert_eq!(
            entry.get("sha256").and_then(toml::Value::as_str),
            Some(sha256_hex(&bytes).as_str()),
            "the recorded `sha256` for {path} is not the hash of its bytes"
        );

        let upstream = entry
            .get("upstream_sha256")
            .and_then(toml::Value::as_str)
            .expect("a `[[spec]]` entry records the hash of the unscrubbed upstream bytes (C-25)");
        assert!(
            upstream.len() == 64 && upstream.chars().all(|c| c.is_ascii_hexdigit()),
            "the recorded `upstream_sha256` for {path} is not a SHA-256"
        );

        // The date in the file name is the date of the pull the entry records, not a free label.
        let fetched_at = entry
            .get("fetched_at")
            .and_then(toml::Value::as_str)
            .expect("a `[[spec]]` entry records when it was pulled");
        let day = fetched_at.split('T').next().expect("an RFC 3339 timestamp");
        assert!(
            path.contains(&format!("-{day}.openapi.yaml")),
            "{path} is named for a different day than the `fetched_at` it records ({fetched_at})"
        );

        recorded.insert(
            Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a UTF-8 file name")
                .to_owned(),
        );
    }

    let present: BTreeSet<String> = vendored.iter().map(|(name, _)| name.clone()).collect();
    assert_eq!(
        recorded, present,
        "the provenance file and `specs/babelforce/` describe different sets of documents"
    );

    // At least one document must differ from what upstream served, or the pair of hashes is
    // recording the same bytes twice and the scrub did nothing. Not required of *every* document:
    // three of the five carry no credential-shaped example at all, and for those the two hashes are
    // equal precisely because nothing had to be removed.
    let scrubbed = specs
        .iter()
        .filter(|entry| entry.get("sha256") != entry.get("upstream_sha256"))
        .count();
    assert!(
        scrubbed > 0,
        "every vendored document hashes identically to its upstream bytes, so nothing was scrubbed \
         — but the redaction list says otherwise. One of the two is wrong."
    );
}

/// A provenance entry is a [`SpecSource`] plus two fields of its own, and `source_url` is absent by
/// decision rather than by oversight.
///
/// Checked by *deserialising* the entry into `SpecSource` after dropping the two extras: `SpecSource`
/// denies unknown fields, so this fails the moment a field is renamed to something the IR would not
/// accept — which is what would make this file provenance-shaped rather than actual provenance.
#[test]
fn a_provenance_entry_is_spec_source_shaped_and_names_no_internal_url() {
    for entry in provenance_specs() {
        let mut narrowed = entry.clone();
        narrowed.remove("name");
        narrowed.remove("upstream_sha256");

        let rendered = toml::to_string(&narrowed).expect("a provenance entry re-renders");
        let source: SpecSource = toml::from_str(&rendered).unwrap_or_else(|error| {
            panic!("a provenance entry is not `SpecSource`-shaped: {error}\n{rendered}")
        });

        assert!(
            source.source_url.is_none(),
            "a provenance entry carries a `source_url`. The only URL there is to record points at an \
             internal GitLab host; `SpecSource::source_url` is `Option` precisely so a document whose \
             origin cannot be published can still be provenanced. `upstream_sha256` is what keeps \
             drift detectable without it."
        );
        assert!(
            source.sha256.is_some() && source.fetched_at.is_some(),
            "a provenance entry that records neither a hash nor a pull date identifies nothing"
        );
    }
}
