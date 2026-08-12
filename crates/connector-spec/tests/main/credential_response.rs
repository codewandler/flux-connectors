//! **No shipped operation returns a secret** (C-430), and the check that says so is one a connector
//! *declares* into rather than one a regex guesses at.
//!
//! `AGENTS.md` § Authentication contract states the rule and this file is the half of it that runs:
//! an operation whose declared response carries a token is withheld until C-136's diversion lands,
//! because the host's redactor holds only values the host itself resolved and cannot know a secret
//! minted by the very call returning it.
//!
//! # Why the gate reads a declaration and not a field name
//!
//! A catalogue-wide scan for token-shaped property names returned **31** hits and **28** of them
//! were correct as they stood, each documented as harmless by its own connector: babelforce's
//! `sessionId` is a call-session identifier, Klaviyo's `public_api_key` is *"public by design — it
//! is embedded in the account's own web pages"*, Typeform's `token` is *"this response's own opaque
//! id"*, Zendesk's `authenticity_token` is *"not a credential for this API"*, Okta's `credentials`
//! *"never carries a password or a secret value"*, and Anthropic's `max_input_tokens` is a limit.
//! A name-shaped gate fails every one of those, and a gate that is wrong nine times in ten teaches
//! authors to route around it. So the rule is about what the value **is**, and only the connector
//! can say that: [`Operation::credential_response`] is the declaration, and this is what refuses it.
//!
//! What the declaration does **not** do is catch an author who never makes it. That limit is real
//! and is why [`WITHHELD`] below exists as well: the four operations C-430 removed are named here
//! with their reasons, so reinstating one silently is a red build rather than a quiet regression.
//! The two halves cover different mistakes — the declaration covers the honest author writing a new
//! connector, the register covers the reinstatement of a known one.
//!
//! # The nesting is the part that must work
//!
//! The first hand-run scan for this walked `properties` one level deep and found three violations.
//! The second walked nested schemas and found a fourth — Postmark's `ApiTokens`, an array of live
//! tokens in plaintext sitting under `Servers[]`, and the worst of the set. A check that cannot
//! reach inside an array of objects would have shipped it, so
//! [`a_credential_inside_an_array_of_objects_is_reachable`] pins exactly that case, in both
//! directions: the nested spelling resolves and the one-level spelling does not.

use std::collections::BTreeSet;

use connector_spec::{response_location_exists, JsonSchema};
use serde_json::json;

use crate::shipped_provider;

/// **The four operations C-430 withheld, each with the reason that withheld it.**
///
/// The same three-category accounting `providers/babelforce.toml` uses — emitted, inexpressible,
/// withheld — carried into a place a build can read, because an exclusion recorded only in prose is
/// an exclusion nothing checks. babelforce's own is checked against its vendored documents by
/// `babelforce_coverage.rs`; the two connectors below are hand-authored with no document to count
/// against, so this register is what plays that part for them.
///
/// **No provider name appears here, deliberately.** The check below asks every definition in
/// `providers/` rather than the one that used to declare the operation, which is both the stronger
/// question — nothing may reintroduce these ids anywhere — and what keeps
/// `shipped_providers_build.rs`'s guard against a hand-maintained provider list satisfied. The
/// provider set is derived; only the withheld ids are written down.
///
/// **Reinstating one of these is a deliberate act.** C-136 is what licenses it: an operation that
/// legitimately produces a credential returns a *handle*, not the secret. When it lands, delete the
/// entry here in the same commit that restores the operation — the same rule
/// `providers/babelforce.toml` states for its own withheld prefixes.
const WITHHELD: &[(&str, &str)] = &[
    (
        "postmark-server-list",
        "every entry's `Servers[].ApiTokens` is that server's own live Server Token(s) in \
         plaintext — the Account API's own mechanism for retrieving one, for every server on the \
         account",
    ),
    (
        "postmark-server-get",
        "`ApiTokens` is the server's own live Server Token(s) in plaintext",
    ),
    (
        "zoom-meeting-get",
        "`start_url` embeds the host's ZAK token: anyone holding the URL starts the meeting as its \
         host",
    ),
    (
        "zoom-meeting-create",
        "`start_url` embeds the host's ZAK token, on the operation that mints the meeting",
    ),
];

/// Every shipped definition, read from `providers/` rather than listed — a list would drift in
/// exactly one direction, and this file exists to stop that direction.
fn shipped() -> BTreeSet<String> {
    std::fs::read_dir(shipped_provider::providers_dir())
        .expect("the repository's providers/ directory is readable")
        .map(|entry| entry.expect("readable directory entry").file_name())
        .filter_map(|name| {
            name.to_string_lossy()
                .strip_suffix(".toml")
                .map(str::to_owned)
        })
        .collect()
}

/// **None of the four ships, from any connector**, read through the same loader a build uses rather
/// than by grepping the file — an operation can arrive from a vendored document as well as from an
/// `[[operations]]` block, and a grep would see only one of those routes.
#[test]
fn no_withheld_operation_is_in_the_shipped_catalogue() {
    for provider in shipped() {
        let connector = shipped_provider::connector(&provider);
        for (operation, reason) in WITHHELD {
            let shipped = connector
                .operations
                .iter()
                .any(|candidate| candidate.id == *operation);

            assert!(
                !shipped,
                "providers/{provider}.toml ships {operation:?}, which C-430 withheld because \
                 {reason}. An operation whose response carries a credential is withheld until \
                 C-136's diversion lands (`AGENTS.md` § Authentication contract). If C-136 has \
                 landed, delete this entry in the same commit that restores the operation"
            );
        }
    }
}

/// **The withheld operations are named where a reader of the connector will meet them**, not just
/// removed. An absence with no reason beside it reads as an oversight, and the next author re-adds
/// it — which is exactly how `zoom-meeting-get` survived C-79 being filed against it.
#[test]
fn every_withheld_operation_is_recorded_in_a_provider_file() {
    let definitions: Vec<String> = shipped()
        .iter()
        .map(|provider| shipped_provider::sources(provider).definition)
        .collect();

    for (operation, _) in WITHHELD {
        assert!(
            definitions
                .iter()
                .any(|definition| definition.contains(operation)),
            "no provider definition names {operation:?} anywhere. C-430 withheld it, and a \
             withheld operation is recorded as a named exclusion with its reason — the \
             three-category accounting (emitted / inexpressible / withheld) babelforce already uses"
        );
    }
}

/// **The case a one-level scan already missed once.**
///
/// Postmark returns `{"Servers": [{..., "ApiTokens": ["<live token>"]}]}`. The credential is two
/// hops down and one of them is through an array, so the resolver has to walk `items` as well as
/// `properties`. Both directions are asserted: the nested spelling resolves, and the one-level
/// spelling a shallow scan would have used does not — otherwise this test would pass against a
/// resolver that says yes to everything.
#[test]
fn a_credential_inside_an_array_of_objects_is_reachable() {
    let schema: JsonSchema = json!({
        "type": "object",
        "required": ["TotalCount", "Servers"],
        "properties": {
            "TotalCount": { "type": "integer" },
            "Servers": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "ID": { "type": "integer" },
                        "ApiTokens": { "type": "array", "items": { "type": "string" } }
                    }
                }
            }
        }
    });

    assert!(
        response_location_exists(&schema, "/Servers/*/ApiTokens"),
        "the resolver must walk into an array of objects — `ApiTokens` under `Servers[]` is the \
         plaintext-token array a one-level scan over `properties` missed on its first pass"
    );
    assert!(
        !response_location_exists(&schema, "/ApiTokens"),
        "a one-level spelling must not resolve, or the resolver is answering yes to everything and \
         proves nothing about the nested case above"
    );
    assert!(
        !response_location_exists(&schema, "/Servers/ApiTokens"),
        "an array must be walked with `*`; accepting a segment that skips the element level would \
         make two spellings mean one thing and neither of them checkable"
    );
    assert!(
        !response_location_exists(&schema, "/Servers/*/ApiToken"),
        "a renamed field must stop resolving — that is what makes the loud error worth having"
    );
}

/// The simple case beside the nested one: Zoom's `start_url` is a root property, and the same
/// resolver reads it. Kept because the two are the shapes the rule has actually met, and a
/// resolver that only handled nesting would be as wrong as one that only handled the root.
#[test]
fn a_credential_at_the_response_root_is_reachable() {
    let schema: JsonSchema = json!({
        "type": "object",
        "required": ["id", "join_url"],
        "properties": {
            "id": { "type": "integer" },
            "join_url": { "type": "string" },
            "start_url": { "type": "string" }
        }
    });

    assert!(response_location_exists(&schema, "/start_url"));
    assert!(!response_location_exists(&schema, "/starturl"));
    assert!(
        !response_location_exists(&schema, "start_url"),
        "a location is a JSON Pointer and must start with `/`; accepting a bare name would admit \
         two spellings of one thing"
    );
}
