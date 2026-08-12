//! **The Anthropic `admin` service's read surface, pinned by name** — C-441.
//!
//! `providers/anthropic.toml` ships a curated slice of the Admin API. C-122 shipped three reads;
//! this story adds the six the Admin API also publishes — who is in the organization, who is in a
//! workspace, one workspace by id, and which invites are outstanding. What makes that a *surface*
//! rather than six independent additions is that the set is asserted **exactly**, so the count and
//! the names both move only when somebody means them to.
//!
//! # Why the set and not a count
//!
//! `babelforce_coverage.rs` is the shape, and its reasoning carries over unchanged: a count says a
//! number moved and not *which* operation moved, so a rename that swaps one id for another at a
//! constant total reads as green. The comparison below is between two **sets of operation id**, and
//! the difference is enumerated in the failure message.
//!
//! # The part this file exists to hold: personal data
//!
//! Organization members, workspace members and invites carry the names and email addresses of real
//! people. This repository's convention — `providers/bitbucket.toml` and `providers/discord.toml`
//! carry the sentence verbatim six times between them — is that every such field says so in the
//! `description`, which is the text a **model** receives rather than UI copy. A convention held only
//! by review is one edit from being gone, so [`PERSONAL_DATA_LOCATIONS`] names each field by JSON
//! Pointer and [`the_fields_that_name_or_contact_a_person_say_so`] refuses a schema that drops the
//! sentence from one.
//!
//! Note what is deliberately *not* asserted: that the list is complete. No test can know that a
//! future field carries personal data. What it can do — and does — is make removing the marking
//! from a known one a red build rather than a silent regression.
//!
//! # Scope
//!
//! Per `AGENTS.md` § "A per-provider test asserts about its provider, never about the catalogue",
//! this file loads `anthropic` **by name** and quantifies over nothing else.

use std::collections::BTreeSet;

use connector_spec::{Connector, HttpMethod, Idempotency, Operation, Risk};

use crate::shipped_provider;

/// The service under test. Its credential and its `[[services]]` entry predate this story.
const ADMIN: &str = "admin";

/// The credential every Admin operation names, overriding the connector's `default_auth`.
const ADMIN_KEY: &str = "anthropic.admin_key";

/// The OAuth2 sibling of [`ADMIN_KEY`], carrying the `org:admin` scope (C-555). An admin operation
/// admits either — they are alternatives, not a pair — and the property this file actually defends
/// is that neither of them is the *regular* key.
const ADMIN_OAUTH: &str = "anthropic.console_oauth_admin";

/// **The whole exposed read surface of the `admin` service**, in file order.
///
/// The first three shipped with C-122; the remaining six are C-441's. Adding an operation to the
/// service without adding it here is a red build, which is the point.
const ADMIN_OPERATIONS: &[&str] = &[
    "anthropic-organization-get",
    "anthropic-organization-members-list",
    "anthropic-organization-member-get",
    "anthropic-workspaces-list",
    "anthropic-workspace-get",
    "anthropic-workspace-members-list",
    "anthropic-workspace-member-get",
    "anthropic-api-keys-list",
    "anthropic-invites-list",
];

/// Every Admin operation that returns a cursor-paginated envelope.
///
/// Each must declare `first_id`, `last_id` and `has_more` in its response schema, and must say in
/// its own `description` that the call is unpaginated — C-30 leaves no query parameter encodable,
/// so this connector cannot feed a cursor back and a caller must not believe it sees the whole
/// organization.
const ADMIN_LISTS: &[&str] = &[
    "anthropic-organization-members-list",
    "anthropic-workspaces-list",
    "anthropic-workspace-members-list",
    "anthropic-api-keys-list",
    "anthropic-invites-list",
];

/// The three cursor fields a vendor list envelope carries.
const CURSOR_FIELDS: &[&str] = &["first_id", "last_id", "has_more"];

/// The sentence this repository marks a personal-data field with, verbatim.
///
/// Taken from `providers/bitbucket.toml` and `providers/discord.toml`, which between them carry it
/// six times. It is one string in one place precisely so that "the convention" is a value a test
/// compares against rather than a habit a reviewer remembers.
const PERSONAL_DATA: &str = "Identifies a named person — read it for what the calling flow needs and do not persist it beyond that";

/// **Every field in the Admin surface whose value identifies or contacts a specific human**, as
/// `(operation id, JSON Pointer into that operation's `response_schema`)`.
///
/// A user's `id` is on the list beside their `email` and `name` deliberately: it is the handle that
/// addresses one person and resolves to both of the others through `anthropic-organization-
/// member-get`, and `providers/discord.toml` already marks a bare `owner_id` on exactly that
/// reasoning. `workspace_id`, `role` and `type` are not on the list — they describe the membership,
/// not the member.
const PERSONAL_DATA_LOCATIONS: &[(&str, &str)] = &[
    (
        "anthropic-organization-members-list",
        "/properties/data/items/properties/id",
    ),
    (
        "anthropic-organization-members-list",
        "/properties/data/items/properties/email",
    ),
    (
        "anthropic-organization-members-list",
        "/properties/data/items/properties/name",
    ),
    ("anthropic-organization-member-get", "/properties/id"),
    ("anthropic-organization-member-get", "/properties/email"),
    ("anthropic-organization-member-get", "/properties/name"),
    (
        "anthropic-workspace-members-list",
        "/properties/data/items/properties/user_id",
    ),
    ("anthropic-workspace-member-get", "/properties/user_id"),
    (
        "anthropic-invites-list",
        "/properties/data/items/properties/email",
    ),
];

fn anthropic() -> Connector {
    shipped_provider::connector("anthropic")
}

/// The operation with this id, or a panic naming what the service does have.
fn operation<'a>(connector: &'a Connector, id: &str) -> &'a Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| {
            let have: Vec<&str> = connector
                .operations
                .iter()
                .map(|operation| operation.id.as_str())
                .collect();
            panic!("providers/anthropic.toml declares no `{id}`; it declares {have:?}")
        })
}

/// Resolve a JSON Pointer against a schema, or `None` when the location does not exist.
///
/// Deliberately not `serde_json`'s own `pointer` in the failure path: a location that resolves to
/// nothing is the shape a field rename takes, and the caller reports it by name.
fn at<'a>(schema: &'a serde_json::Value, pointer: &str) -> Option<&'a serde_json::Value> {
    schema.pointer(pointer)
}

/// **The headline assertion: the `admin` service exposes exactly this read surface.**
///
/// Both directions fail loudly. An operation added to the service and not to [`ADMIN_OPERATIONS`]
/// is an un-reviewed widening of what an Admin API key reaches; one removed from the service is a
/// published address disappearing, which `AGENTS.md` § Service contract forbids outright.
#[test]
fn the_admin_service_exposes_exactly_the_declared_read_surface() {
    let connector = anthropic();

    let declared: BTreeSet<&str> = connector
        .operations
        .iter()
        .filter(|operation| operation.service == ADMIN)
        .filter(|operation| operation.expose)
        .map(|operation| operation.id.as_str())
        .collect();
    let expected: BTreeSet<&str> = ADMIN_OPERATIONS.iter().copied().collect();

    let unexpected: Vec<&&str> = declared.difference(&expected).collect();
    let missing: Vec<&&str> = expected.difference(&declared).collect();

    assert!(
        unexpected.is_empty() && missing.is_empty(),
        "the `admin` service's exposed operation set has moved.\n  \
         in the connector but not in ADMIN_OPERATIONS: {unexpected:?}\n  \
         in ADMIN_OPERATIONS but not in the connector: {missing:?}"
    );
    assert_eq!(
        declared.len(),
        ADMIN_OPERATIONS.len(),
        "ADMIN_OPERATIONS lists an id twice"
    );
}

/// Every Admin operation is a read, on the admin credential, with the shipped error envelope.
///
/// The four claims are one test because they are one decision — "this is a safe, repeatable,
/// admin-authenticated read" — and a failure names which half broke.
#[test]
fn every_admin_operation_is_an_authenticated_idempotent_read() {
    let connector = anthropic();

    for id in ADMIN_OPERATIONS {
        let operation = operation(&connector, id);

        assert_eq!(
            operation.method,
            HttpMethod::Get,
            "{id} is not a GET; C-441 adds no write to this connector"
        );
        assert_eq!(operation.risk, Risk::Low, "{id} does not declare risk low");
        assert_eq!(
            operation.idempotency,
            Idempotency::Idempotent,
            "{id} does not declare idempotency idempotent"
        );

        let auth = operation
            .auth
            .as_ref()
            .unwrap_or_else(|| panic!("{id} inherits default_auth; it must name {ADMIN_KEY}"));
        let credentials: Vec<&str> = auth
            .iter()
            .flat_map(|requirement| requirement.iter())
            .map(String::as_str)
            .collect();
        assert_eq!(
            credentials,
            vec![ADMIN_KEY, ADMIN_OAUTH],
            "{id} must authenticate with exactly the two admin-privileged credentials, as \
             alternatives: the Admin API key or the org:admin OAuth token"
        );
        // The load-bearing half, stated separately so it survives the list above being extended
        // again: an admin operation must never admit the *regular* key or the workspace-scoped
        // OAuth token. That is the escalation boundary this whole file exists to hold.
        assert!(
            !credentials.contains(&"anthropic.api_key")
                && !credentials.contains(&"anthropic.console_oauth"),
            "{id} admits an unprivileged credential {credentials:?}. The Admin API needs the admin \
             role or the org:admin scope; admitting the model-catalogue credential here would let \
             a token provisioned for reading models read the organization"
        );

        let envelope = operation
            .quirks
            .error_envelope
            .as_ref()
            .unwrap_or_else(|| panic!("{id} declares no [operations.quirks.error_envelope]"));
        assert_eq!(
            envelope.message_pointer, "/error/message",
            "{id} does not use the shipped message pointer"
        );
        assert_eq!(
            envelope.code_pointer.as_deref(),
            Some("/error/type"),
            "{id} does not use the shipped code pointer"
        );
    }
}

/// Every list declares its cursor fields and admits, in the model-facing text, that it is
/// unpaginated.
///
/// The two halves answer different readers. The schema fields tell a caller inspecting the response
/// that `first_id`/`last_id` exist and are inert here; the `description` tells a model *before* it
/// calls that one page is all it will get.
#[test]
fn every_admin_list_declares_its_unusable_cursor_fields() {
    let connector = anthropic();

    for id in ADMIN_LISTS {
        let operation = operation(&connector, id);

        assert!(
            operation.description.to_lowercase().contains("unpaginated"),
            "{id}'s description does not say the call is unpaginated; a caller will believe it \
             saw the whole organization.\n  description: {}",
            operation.description
        );

        let schema = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("{id} declares no response_schema"));

        for field in CURSOR_FIELDS {
            let location = format!("/properties/{field}");
            assert!(
                at(schema, &location).is_some(),
                "{id}'s response schema declares no `{field}`"
            );
        }

        // The two cursor ids are the ones a caller would try to page with, so each must say in its
        // own description that it cannot be fed back here. `has_more` is a boolean and carries the
        // fact by being true, so it is not held to the same wording.
        //
        // Two spellings are accepted because the shipped convention uses both, and matching it
        // beats imposing a third: `anthropic-models-list` says its `first_id` "cannot" be used and
        // its `last_id` is "likewise unusable". The claim under test is that the field says it is
        // inert for paging here — not which of the two sentences says it.
        for field in ["first_id", "last_id"] {
            let location = format!("/properties/{field}/description");
            let text = at(schema, &location)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("{id}'s `{field}` carries no description"));
            assert!(
                text.contains("cannot") || text.contains("unusable"),
                "{id}'s `{field}` does not say this connector cannot page with it.\n  \
                 description: {text}"
            );
        }
    }
}

/// **Every field naming or contacting a person carries the repository's personal-data sentence.**
///
/// The Acceptance item this file was written for. A missing location and a missing sentence are
/// reported differently, because they are different mistakes: the first is a field that was renamed
/// or dropped, the second is a field that shipped unmarked.
#[test]
fn the_fields_that_name_or_contact_a_person_say_so() {
    let connector = anthropic();

    for (id, pointer) in PERSONAL_DATA_LOCATIONS {
        let operation = operation(&connector, id);
        let schema = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("{id} declares no response_schema"));

        let field = at(schema, pointer).unwrap_or_else(|| {
            panic!(
                "{id}'s response schema has nothing at `{pointer}` — a personal-data field was \
                 renamed or removed without updating PERSONAL_DATA_LOCATIONS"
            )
        });
        let description = field
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{id}'s `{pointer}` carries no description at all"));

        assert!(
            description.contains(PERSONAL_DATA),
            "{id}'s `{pointer}` is personal data and does not carry the sentence a model is meant \
             to read.\n  expected to contain: {PERSONAL_DATA}\n  actual: {description}"
        );
    }
}

/// **No example person appears anywhere in this connector.**
///
/// `providers/docusign.toml` records the rule and the reason: a placeholder name or address is
/// copied, and a field whose whole content is personal data earns no illustration. The Admin API's
/// own reference prints `Jane Doe` and `user@emaildomain.com`; neither reaches this repository.
#[test]
fn no_example_person_is_invented_anywhere_in_the_file() {
    let definition = shipped_provider::sources("anthropic").definition;

    assert!(
        !definition.contains('@'),
        "providers/anthropic.toml contains an `@`, which is the shape an example email address \
         takes; personal-data fields in this repository carry no example value"
    );
    for invented in ["Jane Doe", "John Doe", "emaildomain"] {
        assert!(
            !definition.contains(invented),
            "providers/anthropic.toml contains `{invented}` — an invented person from the vendor's \
             own reference"
        );
    }
}

/// The connector as a whole stays read-only.
///
/// Scoped to the connector rather than the `admin` service because the charter claim in the file's
/// header is about the whole file, and because a write landing in `models` would be the same
/// mistake wearing a different service name.
#[test]
fn the_anthropic_connector_declares_no_write() {
    let connector = anthropic();

    let writes: Vec<(&str, HttpMethod)> = connector
        .operations
        .iter()
        .filter(|operation| operation.method != HttpMethod::Get)
        .map(|operation| (operation.id.as_str(), operation.method))
        .collect();

    assert!(
        writes.is_empty(),
        "providers/anthropic.toml is read-only by charter (see its header comment, and C-441's \
         Scope section); these operations are not GETs: {writes:?}"
    );
}
