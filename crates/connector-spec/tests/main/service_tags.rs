//! Tags: what **kind** of thing a service is, and the ways declaring one is refused — C-153.
//!
//! A tag is deliberately **not** a role. A role answers "can this service *do* X, checkably?" and
//! carries required members the loader verifies; a tag answers "what kind of thing is this?" and
//! carries nothing. Giving `storage` a required-member list would be meaningless — no operation makes
//! a service storage — and letting a role carry no members would turn every role into an unchecked
//! assertion, which is exactly what [`Role`](connector_spec::Role)'s closed set exists to prevent.
//! So they are two fields with two different guarantees, and this file tests the weaker one.
//!
//! What a tag still refuses:
//!
//! 1. an unknown name — the whole reason the vocabulary is closed, because a typo'd tag silently
//!    means "absent from that filter", which no consumer can distinguish from "genuinely not that
//!    kind of thing";
//! 2. the same tag twice on one service;
//! 3. a provider-level `tags` key — a provider's tags are *derived*, never authored.
//!
//! And two properties the fleet itself must satisfy, so the filter cannot pass vacuously.
//!
//! See [`docs/designs/provider-roles.md`](../../../docs/designs/provider-roles.md) §tags.

use connector_spec::{Connector, Tag};

use crate::shipped_provider;

/// Every provider this repository ships, loaded through the real loader.
///
/// Read from `providers/` rather than listed here, for the reason `shipped_providers.rs` records: a
/// constant naming the current ids is a second source of truth, and it drifts in exactly one
/// direction — a provider lands and the gate silently stops covering it.
fn fleet() -> Vec<Connector> {
    let dir = shipped_provider::providers_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "toml").then(|| path.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    names.sort();
    assert!(
        !names.is_empty(),
        "providers/ is empty, so every gate here would pass vacuously"
    );
    names
        .iter()
        .map(|name| shipped_provider::connector(name))
        .collect()
}

/// A minimal well-formed provider, with `body` spliced in after the connector-level keys.
fn provider(body: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"
description = "A provider that exists to be checked."
{body}
"#
    )
}

/// One operation, in `service`, named `id`.
fn operation(id: &str, service: &str) -> String {
    format!(
        r#"
[[operations]]
id = "{id}"
service = "{service}"
method = "GET"
direction = "read"
path = "/v1/things"
description = "Fetch things."
risk = "low"
idempotency = "idempotent"
"#
    )
}

fn load(source: &str) -> connector_spec::Result<Connector> {
    connector_spec::provider::load("providers/acme.toml", source).map(|loaded| loaded.connector)
}

/// The rendered refusal, or a panic naming the connector that was wrongly accepted.
fn refusal(source: &str) -> String {
    match load(source) {
        Ok(_) => panic!("the loader accepted a provider it must refuse:\n{source}"),
        Err(error) => error.to_string(),
    }
}

/// **The failing-first test of C-153.** An unknown tag is refused, and the message lists the set.
///
/// This is the failure mode the closed vocabulary exists for. A tag carries no required members, so
/// nothing else in the system can notice that `telephny` is not `telephony` — it simply never
/// matches a filter, and a service that looks tagged is invisible to the one query it was tagged
/// for. The refusal has to quote what was written *and* name the alternatives, or an author cannot
/// act on it.
#[test]
fn an_unknown_tag_is_refused_and_names_the_known_set() {
    let source = provider(&format!(
        r#"
[[services]]
name = "voice"
tags = ["telephny"]
{}"#,
        operation("acme-call-list", "voice")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("telephny"),
        "the refusal must quote the name that was written, but said: {error}"
    );
    assert!(
        error.contains("telephony"),
        "the refusal must name the known set so an author can act on it, but said: {error}"
    );
}

/// The same tag twice on one service is refused: a set that tolerates repeats is a list pretending.
#[test]
fn a_repeated_tag_on_one_service_is_refused() {
    let source = provider(&format!(
        r#"
[[services]]
name = "voice"
tags = ["telephony", "telephony"]
{}"#,
        operation("acme-call-list", "voice")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("telephony"),
        "the refusal must name the repeated tag, but said: {error}"
    );
}

/// A provider-level `tags` key is refused — a provider's tags are derived from its services'.
///
/// Same rule `roles` and `Level` already follow: a value that is both derived and writable is two
/// sources of truth waiting to disagree, and it is also the wrong *level*. `google` is not one kind
/// of thing; its `gmail` is email and its `drive` is storage.
#[test]
fn a_provider_level_tags_key_is_refused() {
    let source = provider(&format!(
        r#"
tags = ["telephony"]
[[services]]
name = "voice"
{}"#,
        operation("acme-call-list", "voice")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("tags"),
        "the refusal must name the key that may not be authored, but said: {error}"
    );
}

/// A service carries a **set**, not one value — twilio is messaging and voice at once.
#[test]
fn a_service_may_carry_several_tags() {
    let source = provider(&format!(
        r#"
[[services]]
name = "voice"
tags = ["telephony", "messaging"]
{}"#,
        operation("acme-call-list", "voice")
    ));

    let connector = load(&source).expect("a service carrying two tags must load");
    let service = connector
        .services
        .iter()
        .find(|service| service.name == "voice")
        .expect("the service must survive the load");
    assert_eq!(service.tags, vec![Tag::Telephony, Tag::Messaging]);
}

/// A provider's tags are the **union** of its services', deduplicated, in declaration order.
#[test]
fn a_providers_tags_are_the_union_of_its_services() {
    let source = provider(&format!(
        r#"
[[services]]
name = "mail"
tags = ["email"]

[[services]]
name = "files"
tags = ["storage"]

[[services]]
name = "more-mail"
tags = ["email"]
{}{}{}"#,
        operation("acme-mail-list", "mail"),
        operation("acme-files-list", "files"),
        operation("acme-more-mail-list", "more-mail"),
    ));

    let connector = load(&source).expect("a multi-service provider must load");
    assert_eq!(connector.tags(), vec![Tag::Email, Tag::Storage]);
}

/// **Service-level tagging is only real if some provider's services actually diverge.**
///
/// Otherwise the field would be provider-level in all but name, and the whole reason it hangs off a
/// service — `google`'s `gmail` is email while its `drive` is storage — would be untested. Asserted
/// against the shipped fleet, not a fixture: a synthetic case would prove the mechanism and not the
/// catalogue.
#[test]
fn some_shipped_provider_has_services_whose_tags_diverge() {
    let fleet = fleet();

    let diverging: Vec<&str> = fleet
        .iter()
        .filter(|connector| {
            connector
                .services
                .iter()
                .any(|service| service.tags.len() < connector.tags().len())
                && connector.services.len() > 1
        })
        .map(|connector| connector.id.as_str())
        .collect();

    assert!(
        !diverging.is_empty(),
        "no shipped provider has a service whose tags are narrower than the provider's union, so \
         nothing proves tags belong on a service rather than on a provider"
    );
}

/// The fleet's tag set is **non-empty and multi-valued**, so a filter cannot pass vacuously.
///
/// A catalogue where every service carried the same single tag would satisfy every other test here
/// and be useless: the filter would have one bucket. This is the property that makes the feature a
/// feature rather than a field.
#[test]
fn the_shipped_fleet_uses_several_distinct_tags() {
    let fleet = fleet();

    let mut distinct: Vec<Tag> = Vec::new();
    let mut untagged: Vec<&str> = Vec::new();
    for connector in &fleet {
        let tags = connector.tags();
        if tags.is_empty() {
            untagged.push(connector.id.as_str());
        }
        for tag in tags {
            if !distinct.contains(&tag) {
                distinct.push(tag);
            }
        }
    }

    assert!(
        untagged.is_empty(),
        "every shipped provider must carry at least one tag, but these carry none: {untagged:?}"
    );
    assert!(
        distinct.len() >= 10,
        "the fleet must span many domains or the filter has nothing to filter, but it uses only \
         {} distinct tag(s): {distinct:?}",
        distinct.len()
    );
}
