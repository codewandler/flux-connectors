//! Fixed-byte and scrub gates for the three public Zendesk OpenAPI documents vendored by C-459.
//!
//! The source is public and the repository is public, but that does not make example credentials,
//! personal addresses, or telephone numbers appropriate repository history. The vendor script owns
//! the transformation; these tests inspect the committed result so a stale or bypassed script fails
//! closed.

use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use connector_spec::sha256_hex;
use serde_json::Value;

const DOCUMENTS: [(&str, &[&str]); 3] = [
    ("ticketing", &["basicAuth"]),
    ("help-center", &["basicAuth"]),
    ("messaging", &["basicAuth", "bearerAuth"]),
];

const SOURCE_URLS: [(&str, &str); 3] = [
    (
        "ticketing",
        "https://developer.zendesk.com/zendesk/oas.yaml",
    ),
    (
        "help-center",
        "https://developer.zendesk.com/help_center/oas.yaml",
    ),
    (
        "messaging",
        "https://raw.githubusercontent.com/zendesk/sunshine-conversations-api-spec/",
    ),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

fn provenance() -> toml::Table {
    let path = repo_root().join("specs/zendesk.provenance.toml");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("{} is not valid TOML: {error}", path.display()))
}

fn entries() -> Vec<toml::Table> {
    provenance()
        .get("spec")
        .and_then(toml::Value::as_array)
        .expect("zendesk provenance declares [[spec]] entries")
        .iter()
        .map(|entry| {
            entry
                .as_table()
                .expect("each [[spec]] entry is a table")
                .clone()
        })
        .collect()
}

fn field<'a>(entry: &'a toml::Table, name: &str) -> &'a str {
    entry
        .get(name)
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "provenance entry {:?} has no string {name:?}",
                entry.get("name")
            )
        })
}

fn documents() -> BTreeMap<String, (PathBuf, String)> {
    entries()
        .into_iter()
        .map(|entry| {
            let name = field(&entry, "name").to_owned();
            let path = repo_root().join(field(&entry, "path"));
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            (name, (path, text))
        })
        .collect()
}

#[test]
fn exactly_the_three_declared_documents_and_the_named_legacy_fixture_exist() {
    let entries = entries();
    let names: BTreeSet<&str> = entries.iter().map(|entry| field(entry, "name")).collect();
    let expected: BTreeSet<&str> = DOCUMENTS.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, expected, "the provenance document set moved");

    let mut declared: BTreeSet<String> = entries
        .iter()
        .map(|entry| field(entry, "path").to_owned())
        .collect();
    declared.insert("specs/zendesk/2024-06-01-excerpt.json".to_owned());

    let directory = repo_root().join("specs/zendesk");
    let actual: BTreeSet<String> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()))
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.is_file())
        .map(|path| {
            path.strip_prefix(repo_root())
                .expect("a repository-relative Zendesk path")
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(
        actual, declared,
        "an undeclared Zendesk document is vendored"
    );
}

#[test]
fn provenance_names_public_sources_versions_times_and_both_hashes() {
    for entry in entries() {
        let name = field(&entry, "name");
        let source = field(&entry, "source_url");
        let expected_source = SOURCE_URLS
            .iter()
            .find_map(|(candidate, source)| (*candidate == name).then_some(*source))
            .unwrap_or_else(|| panic!("unexpected Zendesk document {name:?}"));
        if name == "messaging" {
            assert!(
                source.starts_with(expected_source)
                    && source.ends_with("/openapi.yaml")
                    && !source.contains("/master/"),
                "messaging must name a commit-pinned public source, got {source:?}"
            );
        } else {
            assert_eq!(source, expected_source);
        }

        assert!(!field(&entry, "upstream_version").trim().is_empty());
        let fetched_at = field(&entry, "fetched_at");
        assert!(
            fetched_at.ends_with('Z') && fetched_at.contains('T'),
            "{name} has no UTC fetch timestamp: {fetched_at:?}"
        );

        for hash in [field(&entry, "sha256"), field(&entry, "upstream_sha256")] {
            assert!(
                hash.len() == 64 && hash.chars().all(|ch| ch.is_ascii_hexdigit()),
                "{name} records an invalid SHA-256: {hash:?}"
            );
        }

        let path = repo_root().join(field(&entry, "path"));
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(
            sha256_hex(&bytes),
            field(&entry, "sha256"),
            "{} does not match its vendored hash",
            path.display()
        );
    }
}

#[test]
fn every_document_is_openapi_and_keeps_its_security_declarations() {
    let documents = documents();
    for (name, schemes) in DOCUMENTS {
        let (path, text) = &documents[name];
        let document: Value = serde_norway::from_str(text)
            .unwrap_or_else(|error| panic!("{} is not YAML: {error}", path.display()));
        let ingested = connector_spec::openapi::ingest(text)
            .unwrap_or_else(|error| panic!("{} cannot be ingested: {error}", path.display()));
        assert!(
            !ingested.operations.is_empty(),
            "{} ingests no operations",
            path.display()
        );
        assert!(
            document["openapi"]
                .as_str()
                .is_some_and(|version| version.starts_with("3.0.")),
            "{} is not an OpenAPI 3.0 document",
            path.display()
        );
        let declared = document["components"]["securitySchemes"]
            .as_object()
            .unwrap_or_else(|| panic!("{} lost components.securitySchemes", path.display()));
        for scheme in schemes {
            assert!(
                declared.contains_key(*scheme),
                "{} lost security scheme {scheme:?}",
                path.display()
            );
        }
    }
}

#[test]
fn no_contact_identity_credential_or_telephone_survives() {
    for (name, (path, text)) in documents() {
        for token in text.split(|ch: char| {
            ch.is_whitespace() || matches!(ch, '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']')
        }) {
            let token = token.trim_matches(|ch: char| ",.;:!?`{}".contains(ch));
            if let Some((local, domain)) = token.rsplit_once('@') {
                if token.contains('{') || token.contains('}') {
                    continue;
                }
                let domain = domain.to_ascii_lowercase();
                let reserved =
                    ["example.com", "example.net", "example.org"]
                        .iter()
                        .any(|reserved| {
                            domain == *reserved || domain.ends_with(&format!(".{reserved}"))
                        });
                assert!(
                    local.is_empty() || reserved,
                    "{} keeps a non-reserved contact identity {token:?}",
                    path.display()
                );
            }
        }

        for (line_number, line) in text.lines().enumerate() {
            let Some((key, scalar)) = line.trim().split_once(':') else {
                continue;
            };
            let normalized = key
                .trim_matches(['\'', '"'])
                .to_ascii_lowercase()
                .replace(['_', '-'], "");
            let scalar = scalar.trim().trim_matches(['\'', '"']);
            let digits = scalar.chars().filter(char::is_ascii_digit).count();
            if matches!(
                normalized.as_str(),
                "phone" | "phonenumber" | "msisdn" | "from" | "to"
            ) && scalar.starts_with('+')
                && digits >= 8
            {
                panic!(
                    "{}:{} keeps telephone number {scalar:?}",
                    path.display(),
                    line_number + 1
                );
            }

            let credential_key = matches!(
                normalized.as_str(),
                "apikey"
                    | "token"
                    | "accesstoken"
                    | "refreshtoken"
                    | "secret"
                    | "keysecret"
                    | "password"
                    | "authorization"
            );
            let credential_value = scalar != "redacted-credential"
                && scalar.len() >= 12
                && !scalar.starts_with('{')
                && scalar
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || "._~-+/=".contains(ch));
            assert!(
                !(credential_key && credential_value),
                "{}:{} keeps credential-shaped {key:?} example {scalar:?} in {name}",
                path.display(),
                line_number + 1
            );

            let system_identifier_key = matches!(
                normalized.as_str(),
                "keyid" | "appid" | "clientid" | "integrationid"
            );
            let system_identifier_value = scalar != "redacted-identifier"
                && scalar.len() >= 12
                && scalar.chars().all(|ch| ch.is_ascii_hexdigit());
            assert!(
                !(system_identifier_key && system_identifier_value),
                "{}:{} keeps system identifier {key:?} example {scalar:?} in {name}",
                path.display(),
                line_number + 1
            );

            let opaque_example = normalized == "example"
                && scalar != "redacted-opaque"
                && scalar.len() >= 24
                && scalar.chars().all(|ch| ch.is_ascii_hexdigit());
            assert!(
                !opaque_example,
                "{}:{} keeps an opaque system or credential example {scalar:?} in {name}",
                path.display(),
                line_number + 1
            );

            for token in scalar.split(|ch: char| !ch.is_ascii_digit() && ch != '.') {
                let Ok(ip) = token.parse::<Ipv4Addr>() else {
                    continue;
                };
                let octets = ip.octets();
                let documentation = matches!(
                    octets,
                    [192, 0, 2, _] | [198, 51, 100, _] | [203, 0, 113, _]
                );
                assert!(
                    documentation,
                    "{}:{} keeps system IP address {ip}",
                    path.display(),
                    line_number + 1
                );
            }
        }
    }
}

#[test]
fn no_url_names_a_structurally_non_public_host() {
    for (_, (path, text)) in documents() {
        for word in text.split_whitespace() {
            let Some((_, after_scheme)) = word
                .split_once("https://")
                .or_else(|| word.split_once("http://"))
            else {
                continue;
            };
            let authority = after_scheme
                .split(['/', '?', '#', '"', '\'', ')', ']', '}'])
                .next()
                .unwrap_or_default();
            if authority.contains('{') || authority.is_empty() {
                continue;
            }
            let host = authority
                .rsplit_once('@')
                .map_or(authority, |(_, host)| host)
                .split(':')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let private_ipv4 = host.parse::<Ipv4Addr>().is_ok_and(|ip| {
                ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified()
            });
            assert!(
                host != "localhost"
                    && !host.ends_with(".localhost")
                    && !host.ends_with(".internal")
                    && !host.ends_with(".local")
                    && !private_ipv4,
                "{} names non-public URL host {host:?}",
                path.display()
            );
        }
    }
}
