//! Roles: the capability shape a **service** claims, and the four ways claiming one is refused.
//!
//! A role is the declaration seventeen connectors were missing — `openai` and `openrouter` both list
//! models, `zendesk` and `freshdesk` both show a ticket, and nothing in the IR said so, so nothing
//! could act on it. The point of the mechanism is that the claim is *checked*: `llm_catalogue` is a
//! promise the loader enforces, so a consumer reading the catalogue never has to read the provider's
//! TOML to find out whether it holds.
//!
//! Every rule here is therefore a refusal, and each one has its own test:
//!
//! 1. an unknown role name — the failure mode the whole design exists to prevent, because a typo'd
//!    capability that silently means "no capability" is invisible;
//! 2. a service claiming a role whose required members it does not have;
//! 3. a provider-level `roles` key — a provider's roles are *derived*, never authored;
//! 4. the same role declared twice on one service.
//!
//! See [`docs/designs/provider-roles.md`](../../../docs/designs/provider-roles.md).

use connector_spec::{Connector, Role};

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

/// **The failing-first test of C-120.** A role is a contract; claiming it without satisfying it is a
/// load error that names the member that is missing.
///
/// The service lists models under an id that ends in `fetch`, so nothing fills `llm_catalogue`'s
/// `list` slot. Before the check existed this file did not even parse — `roles` was not a key — which
/// is why the assertion is on the *reason*, not on the mere fact of a rejection: "refused for the
/// wrong cause" and "refused for the right cause" have to be distinguishable.
#[test]
fn a_service_claiming_a_role_it_does_not_satisfy_is_refused() {
    let source = provider(&format!(
        r#"
[[services]]
name = "models"
roles = ["llm_catalogue"]
{}"#,
        operation("acme-models-fetch", "models")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("llm_catalogue"),
        "the refusal must name the role that was claimed, but said: {error}"
    );
    assert!(
        error.contains("\"list\""),
        "the refusal must name the required member that is missing, but said: {error}"
    );
}

/// An unknown role name is refused rather than ignored, and the message lists the roles that exist.
///
/// This is the failure mode the design is built around: a role that silently means "no capability"
/// is worse than no role at all, because a consumer cannot tell the two apart.
#[test]
fn an_unknown_role_name_is_refused_and_names_the_known_set() {
    let source = provider(&format!(
        r#"
[[services]]
name = "models"
roles = ["llm_catalog"]
{}"#,
        operation("acme-models-list", "models")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("llm_catalog"),
        "the refusal must quote the name that was written, but said: {error}"
    );
    for role in Role::ALL {
        assert!(
            error.contains(role.word()),
            "the refusal must list the known role {:?}, but said: {error}",
            role.word()
        );
    }
}

/// A provider's roles are the union of its services', so there is no provider-level `roles` key —
/// and one in a file is refused by pointing at the level that does own it.
///
/// The same rule `Level` follows in `crate::config`: a derived value an author could also state is a
/// value two sources of truth can disagree about.
#[test]
fn a_provider_level_roles_key_is_refused_and_points_at_the_service_level() {
    let source = provider(&format!(
        r#"
roles = ["llm_catalogue"]

[[services]]
name = "models"
{}"#,
        operation("acme-models-list", "models")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("roles") && error.contains("[[services]]"),
        "the refusal must send the author to the service level, but said: {error}"
    );
}

/// The same role twice on one service is refused: a claim stated twice states nothing the first
/// statement did not, and a set that tolerates repeats is a list pretending to be a set.
#[test]
fn a_role_declared_twice_on_one_service_is_refused() {
    let source = provider(&format!(
        r#"
[[services]]
name = "models"
roles = ["llm_catalogue", "llm_catalogue"]
{}"#,
        operation("acme-models-list", "models")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("llm_catalogue") && error.contains("more than once"),
        "the refusal must say the role is repeated, but said: {error}"
    );
}

/// The reserved `default` service may be declared **to carry roles**, because a provider with one API
/// surface has nowhere else to put them.
///
/// It is still refused for anything else — see [`a_default_service_entry_may_carry_nothing_but_roles`]
/// — and it stays the implicit service: the connector is still `is_default_only`, so its artifacts are
/// named `<provider>.flux` rather than `<provider>-default.flux`.
#[test]
fn the_reserved_default_service_may_carry_roles() {
    let source = provider(&format!(
        r#"
[[services]]
name = "default"
roles = ["llm_catalogue"]
{}"#,
        operation("acme-models-list", "default")
    ));

    let connector = load(&source).expect("a `default` entry carrying only roles is accepted");
    assert_eq!(connector.roles(), vec![Role::LlmCatalogue]);
    assert!(
        connector.is_default_only(),
        "declaring `default` to carry roles must not turn a single-surface provider into a \
         multi-service one — that would rename every artifact it emits"
    );
    assert_eq!(connector.service_names(), vec!["default"]);
}

/// …but only when `default` is the provider's **only** service.
///
/// The exception exists because a single-surface provider has nowhere else to put a role. Beside a
/// named service it would hand back the implicit `default` that a multi-service provider must not
/// have, and the harm is concrete rather than doctrinal — see the test below, which is the one that
/// would have caught this being missed.
#[test]
fn a_default_service_entry_beside_a_named_service_is_refused() {
    let source = provider(&format!(
        r#"
[[services]]
name = "default"
roles = ["llm_catalogue"]

[[services]]
name = "chat"
{}{}"#,
        operation("acme-models-list", "default"),
        operation("acme-chat-completion", "chat"),
    ));

    let error = refusal(&source);
    assert!(
        error.contains("chat"),
        "the refusal must name the service the entry sits beside, but said: {error}"
    );
}

/// The harm the rule above prevents: a `default` entry beside a named service would make an operation
/// that omits `service` legal again in a multi-service file.
///
/// That operation would be emitted into a `<provider>-default.flux` nobody declared or asked for —
/// which is exactly what C-49's `validate_operation_service` refuses, and what admitting the entry
/// unconditionally would have repealed.
#[test]
fn declaring_default_does_not_give_a_multi_service_provider_an_implicit_service_back() {
    let with_entry = provider(&format!(
        r#"
[[services]]
name = "default"
roles = ["llm_catalogue"]

[[services]]
name = "chat"
{}
[[operations]]
id = "acme-models-list"
method = "GET"
direction = "read"
path = "/v1/models"
description = "List the models — and name no service."
risk = "low"
idempotency = "idempotent"
"#,
        operation("acme-chat-completion", "chat"),
    ));

    assert!(
        load(&with_entry).is_err(),
        "an operation naming no service must stay refused in a multi-service file, whether or not \
         the file also declares a `default` entry"
    );
}

/// …and nothing else. A `base_url`, an `api_version` or a `description` on the `default` entry would
/// be a second definition of something the connector already states, with nothing to say which one an
/// operation meant. That is the whole reason the name is reserved, and roles are the one thing that
/// has no connector-level spelling to contradict.
#[test]
fn a_default_service_entry_may_carry_nothing_but_roles() {
    let source = provider(&format!(
        r#"
[[services]]
name = "default"
description = "Everything."
roles = ["llm_catalogue"]
{}"#,
        operation("acme-models-list", "default")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("description"),
        "the refusal must name the key that overreached, but said: {error}"
    );
}

/// A `default` entry that carries no roles is refused too: it declares a service that already exists
/// and says nothing new about it, which is exactly what C-49 reserved the name against.
#[test]
fn a_default_service_entry_carrying_no_roles_is_still_refused() {
    let source = provider(&format!(
        r#"
[[services]]
name = "default"
{}"#,
        operation("acme-models-list", "default")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("reserved"),
        "the refusal must still say the name is reserved, but said: {error}"
    );
}

/// A provider's roles are the union of its services', deduplicated, in declaration order.
#[test]
fn a_providers_roles_are_the_union_of_its_services() {
    let source = provider(&format!(
        r#"
[[services]]
name = "models"
roles = ["llm_catalogue"]

[[services]]
name = "mirror"
roles = ["llm_catalogue"]

[[services]]
name = "chat"
{}{}{}"#,
        operation("acme-models-list", "models"),
        operation("acme-mirror-list", "mirror"),
        operation("acme-chat-completion", "chat"),
    ));

    let connector = load(&source).expect("every service satisfies what it claims");
    assert_eq!(
        connector.roles(),
        vec![Role::LlmCatalogue],
        "the union is a set: two services claiming one role contribute it once"
    );
}

/// A required member is matched by its **name within the service** — its trailing name segments —
/// which is what makes a role vendor-independent. `openai-models-list` and `openrouter-models-list`
/// fill the same `list` slot; the vendor prefix nobody agreed on stays out of the contract.
#[test]
fn a_required_member_is_matched_by_its_trailing_segments_not_its_full_id() {
    for id in ["acme-models-list", "acme-list", "list"] {
        let source = provider(&format!(
            r#"
[[services]]
name = "models"
roles = ["llm_catalogue"]
{}"#,
            operation(id, "models")
        ));
        assert!(
            load(&source).is_ok(),
            "{id:?} ends in the `list` segment, so it fills the slot"
        );
    }

    // `-listing` is not the `list` segment: the match is on whole segments, not on a substring, or
    // `acme-blocklist-get` would satisfy a role it has nothing to do with.
    let source = provider(&format!(
        r#"
[[services]]
name = "models"
roles = ["llm_catalogue"]
{}"#,
        operation("acme-models-listing", "models")
    ));
    assert!(
        load(&source).is_err(),
        "a substring is not a segment; `acme-models-listing` must not fill the `list` slot"
    );
}

/// **Only an operation fills a role slot.** An event of the right name does not.
///
/// A role is a claim that something is *callable*: a consumer resolving `llm_catalogue` intends to
/// call the listing. An event is emitted into no module at all — flux lifts `op` declarations only —
/// so a service satisfying `list` with an event would publish a live-listing capability that nothing
/// can call, which is an event dressed up as a pollable op.
#[test]
fn only_an_operation_fills_a_role_slot_and_an_event_does_not() {
    let source = provider(&format!(
        r#"
[[services]]
name = "models"
roles = ["llm_catalogue"]
{}
[[events]]
name = "models.list"
service = "models"
description = "The vendor announces a change to the model list."
"#,
        operation("acme-models-fetch", "models")
    ));

    let error = refusal(&source);
    assert!(
        error.contains("\"list\""),
        "an event named `models.list` must not satisfy the `list` slot, but said: {error}"
    );
    assert!(
        error.contains("[[operations]]"),
        "the refusal must say which member kind can fill a slot, but said: {error}"
    );
}

/// A provider that declares no roles hashes exactly as it did before roles existed.
///
/// The same property `Service`, the inbound members and `Operation::service` each hold through their
/// own `skip_serializing_if`: landing a field must not move every `ir_sha256` in the repository and
/// churn `connectors.lock` for a provider nobody edited.
#[test]
fn a_provider_declaring_no_roles_hashes_as_it_did_before_roles_existed() {
    let source = provider(&format!(
        r#"
[[services]]
name = "models"
{}"#,
        operation("acme-models-list", "models")
    ));

    let connector = load(&source).expect("a service declaring no roles is unremarkable");
    let domain = connector.hash_domain().expect("the hash domain encodes");
    assert!(
        !domain.contains("roles"),
        "an empty `roles` must not reach the hash domain, or every connector's hash moves: {domain}"
    );
}
