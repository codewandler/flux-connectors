//! One statement about many operations: the selector, the naming rule, and risk by selector —
//! C-411, C-412, C-414.
//!
//! `spec_backed_provider.rs` covers the per-operation overlay, which selects **one** `operationId`
//! and states everything about it. That is the right grain for nine operations and the wrong one
//! for 397: at one `[[patch.operations]]` block per operation, babelforce's canonical surface is
//! 397 blocks each carrying a `select`, a `rename`, a `risk` and an `idempotency` before any real
//! correction. This file is the three declarations that make one statement cover a set, and the
//! refusals that keep the set from being a place safety claims go to die.
//!
//! # Why this reads the real vendored documents
//!
//! A hand-cut excerpt would let the selector be tested against paths chosen to suit it. The
//! manager document declares 356 operations over one path space nobody here designed, and the five
//! documents together declare 398 — which is the number the scope constraint is written against, so
//! it is the number the tests are written against too. Synthetic documents appear only where a real
//! one cannot supply the case: an `operationId` that cannot produce a legal name, and an `internal`
//! path segment, of which there are deliberately zero across all five.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use connector_spec::{provider, Connector, HttpMethod, Idempotency, Risk, SpecDocument};

/// The five vendored babelforce documents, as `(service, repository-relative path)`.
///
/// Spelled here rather than globbed: a test that discovered its own inputs would keep passing if a
/// document vanished, and the scope constraint this file checks is a statement about *these five*.
const DOCUMENTS: [(&str, &str); 5] = [
    (
        "manager",
        "specs/babelforce/manager-2026-07-10.openapi.yaml",
    ),
    ("auth", "specs/babelforce/auth-2026-06-25.openapi.yaml"),
    ("user", "specs/babelforce/user-2026-06-25.openapi.yaml"),
    (
        "task-automation",
        "specs/babelforce/task-automation-2026-06-25.openapi.yaml",
    ),
    (
        "task-schedule",
        "specs/babelforce/task-schedule-2026-06-25.openapi.yaml",
    ),
];

/// The canonical surface manager-sdk exposes: every operation the five documents declare, less the
/// webhook receiver.
///
/// **Owner-stated, 2026-08-01.** 398 operations are declared; `POST /api/v1/webhook/zendesk`
/// (`operationId` `zendesk`) is a receiver babelforce calls *into*, not an operation a caller
/// invokes, so it is not part of the surface. Nothing beyond this set is in scope.
const CANONICAL: usize = 397;

/// **How much of the canonical surface a build can reach today, and why it is not all of it.**
///
/// Two causes, and they are different in kind — which is why the accounting below names **three**
/// categories rather than writing the shortfall off as one number.
///
/// - **[`MULTIPART`] — five ingest cannot express.** Ingest (C-4) skips an operation whose request
///   body is `multipart/form-data`, with a diagnostic, rather than emitting it without its body.
///   Selection never sees them: they are not a thing a selector failed to match, they are a thing
///   ingest did not produce.
/// - **[`WITHHELD`] — four withheld by rule.** Three because an authentication endpoint describes
///   *how to authenticate* and is never a connector operation; one because its response *delivers*
///   a credential. Both rules are `AGENTS.md` § Authentication contract, owner-stated 2026-08-01.
///   These *are* expressible; the selection deliberately omits them.
///
/// So `388 + 5 + 4 = 397`, and all three terms are asserted below rather than the shortfall being
/// written off. Whoever teaches the IR to carry a multipart body moves the first term; nothing
/// should ever move the second.
const REACHABLE: usize = 388;

/// The five manager operations ingest cannot express, by path.
///
/// **C-426 established these are not this repository's to close.** flux 0.49 cannot carry a
/// multipart body at all — re-verified against the 0.49.0 sources at the C-455 bump, where
/// `http.request`'s `body` parameter is declared `{"type": "string"}` and
/// read with `Value::as_str`, and `parse`'s `as_type` is a closed list of six — `f64`, `i64`,
/// `bool`, `json`, `string`, `form` — the analyzer rejects anything outside
/// (`flux_lang::analyze`). There is no part list, no per-part filename, no per-part content type
/// and no boundary. Describing the body in the IR would emit a module that fails on a real call, so
/// the five stay named here. The fix is a flux-side encoder, the same shape as the form/query gap
/// `AGENTS.md` records under `zendesk-ticket-search`.
const MULTIPART: [&str; 5] = [
    "/api/v2/agents/provision",
    "/api/v2/agents/provision/validate",
    "/api/v2/outbound/lists/{id}/leads/upload",
    "/api/v2/phonebook/bulk",
    "/api/v2/prompts",
];

/// The four operations withheld because of the credentials they carry, not because of any limit.
///
/// Unlike [`MULTIPART`] these are perfectly expressible — ingest produces them and a selector would
/// match them. They are absent because selecting them would be a selection error. The three
/// `/oauth/*` paths are authentication endpoints rather than operations; `/api/v2/user/account`
/// returns the customer's REST API `accessToken` and the stream `token` in its 200 body. See
/// `AGENTS.md` § Authentication contract and the two commented blocks in `providers/babelforce.toml`.
const WITHHELD: [&str; 4] = [
    "/api/v2/user/account",
    "/oauth/authorize",
    "/oauth/revoke",
    "/oauth/token",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// A spec cache holding all five vendored documents.
///
/// Leaked so a helper can hand out `SpecDocument<'static>` without threading a lifetime through
/// every fixture; a test process is the one place that costs nothing.
fn cache() -> Vec<SpecDocument<'static>> {
    DOCUMENTS
        .iter()
        .map(|(_, path)| SpecDocument {
            path,
            document: Box::leak(read(path).into_boxed_str()),
        })
        .collect()
}

/// A synthetic cache, for the two cases no real document supplies.
fn synthetic(path: &'static str, document: String) -> Vec<SpecDocument<'static>> {
    vec![SpecDocument {
        path,
        document: Box::leak(document.into_boxed_str()),
    }]
}

fn load_from(definition: &str, documents: &[SpecDocument<'_>]) -> Connector {
    provider::load_with_spec("providers/babelforce.toml", definition, documents)
        .unwrap_or_else(|error| panic!("this definition was expected to load:\n{error}"))
        .connector
}

fn load(definition: &str) -> Connector {
    load_from(definition, &cache())
}

fn refuse_from(definition: &str, documents: &[SpecDocument<'_>]) -> String {
    provider::load_with_spec("providers/babelforce.toml", definition, documents)
        .err()
        .unwrap_or_else(|| panic!("this definition was expected not to load:\n{definition}"))
        .to_string()
}

fn refuse(definition: &str) -> String {
    refuse_from(definition, &cache())
}

fn ids(connector: &Connector) -> Vec<&str> {
    connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect()
}

fn operation<'a>(connector: &'a Connector, id: &str) -> &'a connector_spec::Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("no operation {id:?} among {:?}", ids(connector)))
}

/// The connector header every fixture below carries: five documents, five services.
const POINTER: &str = r#"
id = "babelforce"
vendor = "Babelforce"
base_url = "https://services.babelforce.com"

[[services]]
name = "manager"
description = "The manager API"

[[services]]
name = "auth"
description = "OAuth token endpoints"

[[services]]
name = "user"
description = "The signed-in user"

[[services]]
name = "task-automation"
description = "Task automation"

[[services]]
name = "task-schedule"
description = "Task schedules"

[[spec]]
path = "specs/babelforce/manager-2026-07-10.openapi.yaml"
service = "manager"

[[spec]]
path = "specs/babelforce/auth-2026-06-25.openapi.yaml"
service = "auth"

[[spec]]
path = "specs/babelforce/user-2026-06-25.openapi.yaml"
service = "user"

[[spec]]
path = "specs/babelforce/task-automation-2026-06-25.openapi.yaml"
service = "task-automation"

[[spec]]
path = "specs/babelforce/task-schedule-2026-06-25.openapi.yaml"
service = "task-schedule"

[[auth]]
name = "babelforce.access_token"
scheme = "bearer"
env = ["BABELFORCE_ACCESS_TOKEN"]
description = "SSO-issued babelforce access token"

[patch.naming]
rule = "kebab"
prefix = "babelforce"
"#;

fn with(patch: &str) -> String {
    let definition = read("providers/babelforce.toml");
    let start = definition
        .find("[patch.directions.manager]")
        .expect("the shipped provider carries reviewed directions");
    let end = definition[start..]
        .find("[patch.naming]")
        .map(|offset| start + offset)
        .expect("the reviewed direction maps precede naming");
    format!("{POINTER}{}{patch}", &definition[start..end])
}

/// A deliberately transport-adversarial source: the write starts as `GET` and the read starts as
/// `POST`. Tests swap only those upstream method keys while preserving service and `operationId`.
fn direction_document(flush_method: &str, lookup_method: &str, flush_id: &str) -> String {
    format!(
        r#"{{
  "openapi": "3.0.3",
  "info": {{ "title": "Direction", "version": "1" }},
  "servers": [{{ "url": "https://api.acme.test" }}],
  "paths": {{
    "/v1/widgets/flush": {{
      "{flush_method}": {{
        "operationId": "{flush_id}",
        "summary": "Flush queued work",
        "responses": {{ "200": {{ "description": "ok" }} }}
      }}
    }},
    "/v1/widgets/lookup": {{
      "{lookup_method}": {{
        "operationId": "lookupWidgets",
        "summary": "Look up widgets without changing them",
        "responses": {{ "200": {{ "description": "ok" }} }}
      }}
    }}
  }}
}}"#
    )
}

const DIRECTION_POINTER: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"

[spec]
path = "specs/acme/direction.json"

[patch.directions.default]
flushWidgets = "write"
lookupWidgets = "read"

[patch.naming]
rule = "kebab"
prefix = "acme"

# Both method selectors state the same transport-independent safety metadata. Swapping only an
# upstream method therefore forces selector rematching without giving direction another source.
[[patch.select]]
path_prefix = "/v1"
methods = ["GET"]
risk = "high"
idempotency = "non_idempotent"

[[patch.select]]
path_prefix = "/v1"
methods = ["POST"]
risk = "high"
idempotency = "non_idempotent"
"#;

/// Direction survives the part the lowering-only test cannot reach: ingest and selector
/// composition. The method keys change in the source document before either load, so each operation
/// rematches the opposite selector. Stable service + vendor `operationId` remains the only source
/// of read/write truth. `connector-flux::op_emitter` separately proves these values lower to the
/// matching `read`/`write` effects.
#[test]
fn changing_only_upstream_methods_before_composition_preserves_authored_directions() {
    let original_document = synthetic(
        "specs/acme/direction.json",
        direction_document("get", "post", "flushWidgets"),
    );
    let changed_document = synthetic(
        "specs/acme/direction.json",
        direction_document("post", "get", "flushWidgets"),
    );

    let original = load_from(DIRECTION_POINTER, &original_document);
    let changed = load_from(DIRECTION_POINTER, &changed_document);

    let original_flush = operation(&original, "acme-flush-widgets");
    let changed_flush = operation(&changed, "acme-flush-widgets");
    assert_eq!(original_flush.method, HttpMethod::Get);
    assert_eq!(changed_flush.method, HttpMethod::Post);
    assert_eq!(
        original_flush.direction,
        connector_spec::OperationDirection::Write
    );
    assert_eq!(changed_flush.direction, original_flush.direction);

    let original_lookup = operation(&original, "acme-lookup-widgets");
    let changed_lookup = operation(&changed, "acme-lookup-widgets");
    assert_eq!(original_lookup.method, HttpMethod::Post);
    assert_eq!(changed_lookup.method, HttpMethod::Get);
    assert_eq!(
        original_lookup.direction,
        connector_spec::OperationDirection::Read
    );
    assert_eq!(changed_lookup.direction, original_lookup.direction);
}

/// An upstream identity rename may not inherit the old operation's reviewed truth. The old map row
/// becomes an orphan and the newly selected identity has no direction; both sides are named so a
/// refresh cannot silently promote the renamed operation.
#[test]
fn an_upstream_operation_id_rename_orphans_direction_and_refuses() {
    let renamed = synthetic(
        "specs/acme/direction.json",
        direction_document("get", "post", "flushWidgetsRenamed"),
    );
    let refusal = refuse_from(DIRECTION_POINTER, &renamed);
    assert!(
        refusal.contains("flushWidgets") && refusal.contains("names no `operationId`"),
        "the stale reviewed map key must be reported as an orphan: {refusal}"
    );
    assert!(
        refusal.contains("flushWidgetsRenamed") && refusal.contains("states no `direction`"),
        "the renamed selected operation must fail closed without reviewed truth: {refusal}"
    );
}

// ---------------------------------------------------------------------------------------------
// C-411 · A selector matches a set
// ---------------------------------------------------------------------------------------------

/// **The failing-first test for C-411.** One statement selects twelve operations.
///
/// `/api/v2/agents` + `GET` is the acceptance's own example, and the set is written out rather than
/// counted: a count would keep passing if the selector matched twelve of the wrong operations, and
/// the whole claim is about *which* operations one statement reaches.
#[test]
fn a_selector_matches_by_service_path_prefix_and_method() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
"#,
    ));

    assert_eq!(
        ids(&connector),
        vec![
            "babelforce-list-agents",
            "babelforce-list-agent-groups",
            "babelforce-list-agents-in-group",
            "babelforce-get-agent-group",
            "babelforce-list-all-agent-logs",
            "babelforce-list-agent-presences",
            "babelforce-get-agent-presence",
            "babelforce-export-agents",
            "babelforce-get-agent-import-job",
            "babelforce-list-available-agent-statuses",
            "babelforce-get-agent",
            "babelforce-list-agent-logs",
            "babelforce-get-agent-status",
        ],
        "one selector must reach every GET under the prefix, and nothing else"
    );
    assert!(
        connector
            .operations
            .iter()
            .all(|operation| operation.method == HttpMethod::Get
                && operation.path.starts_with("/api/v2/agents")
                && operation.service == "manager"),
        "a matched operation carries the vendor's own method, path and document service"
    );
}

/// Selection stays opt-in: `hide` is not a key, in either spelling.
///
/// Asserted rather than assumed because the pressure to add one grows with the size of the matched
/// set — and an opt-out list is how every operation a vendor adds upstream becomes a tool by
/// default, learned about from a model's behaviour rather than from a diff.
#[test]
fn there_is_no_hide_key() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
hide = ["listAgents"]
"#,
    ));
    assert!(
        refusal.contains("hide"),
        "an opt-out key must be refused by name: {refusal}"
    );
}

/// A spec-backed file that selects nothing publishes nothing, selectors or no selectors.
#[test]
fn a_spec_backed_provider_with_no_selector_publishes_nothing() {
    let connector = load(POINTER);
    assert!(
        connector.operations.is_empty(),
        "selection is opt-in: {:?}",
        ids(&connector)
    );
}

/// A selector that matches nothing is a loud error — the same rot a `select` naming an absent
/// `operationId` already refuses, one grain up. A prefix that stops matching after an upstream
/// reshuffle must not quietly empty the connector.
#[test]
fn a_selector_that_matches_nothing_is_refused() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/widgets"
methods = ["GET"]
"#,
    ));
    assert!(
        refusal.contains("/api/v2/widgets") && refusal.contains("matches no operation"),
        "a selector matching nothing must name itself: {refusal}"
    );
}

/// `path_prefix` matches whole path segments. `/api/v2/agent` must not reach `/api/v2/agents`.
#[test]
fn a_path_prefix_matches_on_segment_boundaries() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agent"
methods = ["GET"]
"#,
    ));
    assert!(
        refusal.contains("matches no operation"),
        "a prefix must not match half a segment: {refusal}"
    );
}

/// A per-operation block wins over a selector that also matched, field by field.
#[test]
fn a_per_operation_block_wins_over_a_selector() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
risk = "low"
idempotency = "idempotent"
expose = false

[[patch.operations]]
service = "manager"
select = "listAgents"
rename = "babelforce-agent-list"
description = "The curated spelling"
expose = true
"#,
    ));

    let curated = operation(&connector, "babelforce-agent-list");
    assert_eq!(curated.description, "The curated spelling");
    assert!(
        curated.expose,
        "the block's `expose` wins over the selector's"
    );
    // The block says nothing about risk, so the selector's statement stands.
    assert_eq!(curated.risk, Risk::Low);
    assert!(
        !ids(&connector).contains(&"babelforce-list-agents"),
        "the block replaces the selector's derived id rather than publishing beside it: {:?}",
        ids(&connector)
    );
    assert!(
        !operation(&connector, "babelforce-get-agent").expose,
        "everything the block did not name keeps the selector's exposure"
    );
}

/// A broad selector may be narrowed by an exact, reason-bearing deferral. The operation remains
/// claimed so the selector pass cannot publish it again.
#[test]
fn an_exact_deferral_withholds_one_selector_match() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]

[[patch.operations]]
service = "manager"
select = "listAgents"
defer = "Its array query parameter has no declared wire convention."
"#,
    ));

    assert!(!ids(&connector).contains(&"babelforce-list-agents"));
    assert!(ids(&connector).contains(&"babelforce-get-agent"));
}

/// Deferral narrows an explicit set; it is not another spelling of opt-out selection.
#[test]
fn deferring_an_operation_no_selector_matched_is_refused() {
    let refusal = refuse(&with(
        r#"
[[patch.operations]]
service = "manager"
select = "listAgents"
defer = "Its array query parameter has no declared wire convention."
"#,
    ));
    assert!(
        refusal.contains("listAgents") && refusal.contains("[[patch.select]]"),
        "an unmatched deferral must say which explicit selection is missing: {refusal}"
    );
}

#[test]
fn a_deferral_reason_must_be_nonempty() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]

[[patch.operations]]
service = "manager"
select = "listAgents"
defer = "   "
"#,
    ));
    assert!(
        refusal.contains("listAgents") && refusal.contains("non-empty reason"),
        "an empty reason must not make a disappearance review-proof: {refusal}"
    );
}

#[test]
fn a_deferred_operation_cannot_also_be_corrected() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]

[[patch.operations]]
service = "manager"
select = "listAgents"
defer = "Its array query parameter has no declared wire convention."
rename = "babelforce-agent-list"
"#,
    ));
    assert!(
        refusal.contains("listAgents") && refusal.contains("rename"),
        "a correction to an operation that will not publish must be refused: {refusal}"
    );
}

/// Two selectors may overlap while they agree.
#[test]
fn overlapping_selectors_that_agree_are_accepted() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
risk = "low"

[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents/{id}"
methods = ["GET"]
risk = "low"
"#,
    ));
    assert_eq!(
        ids(&connector).iter().collect::<BTreeSet<_>>().len(),
        connector.operations.len(),
        "an operation two selectors matched is published once"
    );
    assert!(ids(&connector).contains(&"babelforce-get-agent"));
}

/// Two selectors that disagree about one operation are refused. Merge order stays total only while
/// no two statements fight over the same field.
#[test]
fn overlapping_selectors_that_disagree_are_refused() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
risk = "low"

[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents/{id}"
methods = ["GET"]
risk = "medium"
"#,
    ));
    assert!(
        refusal.contains("getAgent") && refusal.contains("risk"),
        "a disagreement must name the operation and the field: {refusal}"
    );
}

/// The whole front-end is byte-reproducible: identical inputs, identical IR.
#[test]
fn identical_inputs_produce_identical_ir() {
    let definition = with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET", "DELETE"]
risk = "destructive"
idempotency = "non_idempotent"
"#,
    );
    assert_eq!(load(&definition), load(&definition));
}

/// No operation whose path carries an `internal` segment is ever selected.
///
/// There are zero such paths across the five documents today, so this is a guard against a future
/// pull rather than a description of one — which is exactly why it is tested against a synthetic
/// document. A bulk statement excludes them silently; naming one explicitly is refused, because
/// that is an author asking for it.
#[test]
fn an_internal_path_is_never_selected() {
    let document = synthetic("specs/acme/internal.json", INTERNAL_DOCUMENT.to_owned());
    let pointer = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"

[spec]
path = "specs/acme/internal.json"

[patch.naming]
rule = "kebab"
prefix = "acme"

[patch.directions.default]
listThings = "read"

[[patch.select]]
path_prefix = "/v1"
"#;
    let connector = load_from(pointer, &document);
    assert_eq!(
        ids(&connector),
        vec!["acme-list-things"],
        "an `internal` segment is not selectable in bulk"
    );

    let refusal = refuse_from(
        &format!(
            "{pointer}
[[patch.operations]]
select = \"internalReset\"
rename = \"acme-internal-reset\"
risk = \"destructive\"
idempotency = \"non_idempotent\"
"
        ),
        &document,
    );
    assert!(
        refusal.contains("internalReset") && refusal.contains("internal"),
        "naming an internal operation explicitly must be refused: {refusal}"
    );
}

/// One `internal` path, one ordinary one, and one operation that cannot produce a legal op id.
const INTERNAL_DOCUMENT: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme", "version": "1.0.0" },
  "paths": {
    "/v1/things": {
      "get": { "operationId": "listThings", "summary": "List things", "responses": { "200": { "description": "ok" } } }
    },
    "/v1/internal/reset": {
      "post": { "operationId": "internalReset", "summary": "Reset", "responses": { "200": { "description": "ok" } } }
    }
  }
}"#;

// ---------------------------------------------------------------------------------------------
// C-412 · A naming rule instead of 397 renames
// ---------------------------------------------------------------------------------------------

/// The rule derives the declared spelling, and the acceptance's own example is the case.
#[test]
fn the_naming_rule_derives_the_declared_spelling() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/calls/reporting"
methods = ["GET"]
"#,
    ));
    assert_eq!(
        ids(&connector),
        vec![
            "babelforce-list-reporting-calls",
            "babelforce-list-all-simple-reporting-calls",
            "babelforce-list-dialer-simple-reporting-calls",
            "babelforce-list-inbound-simple-reporting-calls",
            "babelforce-list-outbound-simple-reporting-calls",
        ],
        "`listReportingCalls` derives the declared spelling, and so does everything beside it"
    );
}

/// A pin overrides the rule for one operation, and a per-operation `rename` overrides the pin.
/// Naming precedence is `rename`, then `pin`, then `rule` — total, and stated once.
#[test]
fn a_pin_overrides_the_rule_and_a_rename_overrides_the_pin() {
    let connector = load(&with(
        r#"
[patch.naming.pin]
listAgents = "babelforce-agent-list"
getAgent = "babelforce-agent-get"

[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]

[[patch.operations]]
service = "manager"
select = "getAgent"
rename = "babelforce-agent-fetch"
"#,
    ));
    let published = ids(&connector);
    assert!(
        published.contains(&"babelforce-agent-list"),
        "{published:?}"
    );
    assert!(
        published.contains(&"babelforce-agent-fetch"),
        "a `rename` outranks a pin: {published:?}"
    );
    assert!(!published.contains(&"babelforce-agent-get"));
    assert!(!published.contains(&"babelforce-list-agents"));
}

/// A pin naming an `operationId` no document declares is a loud error.
#[test]
fn a_pin_naming_an_absent_operation_id_is_refused() {
    let refusal = refuse(&with(
        r#"
[patch.naming.pin]
listAgentz = "babelforce-agent-list"

[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
"#,
    ));
    assert!(
        refusal.contains("listAgentz"),
        "a stale pin must name itself: {refusal}"
    );
}

/// Two `operationId`s deriving one op id refuse. Never last-write-wins: an op id is what users and
/// models call by name, so the loser would silently become unreachable.
///
/// The case is real — babelforce declares `getUser` in `manager-2026-07-10` **and** in
/// `user-2026-06-25`, as two different requests — which is why it is the one this asserts.
#[test]
fn two_operation_ids_deriving_one_op_id_refuse() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/users/{id}"
methods = ["GET"]

[[patch.select]]
service = "user"
path_prefix = "/api/v2/user/me"
methods = ["GET"]
"#,
    ));
    assert!(
        refusal.contains("babelforce-get-user") && refusal.contains("getUser"),
        "a collision must name the derived id and both sources: {refusal}"
    );
}

/// An `operationId` that cannot produce a legal op id is reported, naming the operation — never
/// mangled into something that happens to parse.
#[test]
fn an_operation_id_that_cannot_produce_a_legal_name_is_reported() {
    let document = synthetic(
        "specs/acme/illegal.json",
        r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme", "version": "1.0.0" },
  "paths": {
    "/v1/things": {
      "get": { "operationId": "things.list", "summary": "List", "responses": { "200": { "description": "ok" } } }
    }
  }
}"#
        .to_owned(),
    );
    let refusal = refuse_from(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"

[spec]
path = "specs/acme/illegal.json"

[patch.naming]
rule = "kebab"
prefix = "acme"

[[patch.select]]
path_prefix = "/v1"
"#,
        &document,
    );
    assert!(
        refusal.contains("things.list") && refusal.contains("pin"),
        "an underivable name must name the operation and the way out: {refusal}"
    );
}

/// **The full derived id set for a fixture, pinned.**
///
/// `task-automation` declares 31 operations; the selector below reaches the 30 that are not the
/// webhook receiver. Every one of these ids is a public name, and this list is what makes an
/// upstream `operationId` rename move one of them **loudly** rather than quietly — which is the
/// entire reason a naming rule is allowed to exist beside "op ids are a public contract".
#[test]
fn the_derived_id_set_is_pinned() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "task-automation"
path_prefix = "/api/v3"
methods = ["GET"]
risk = "high"
idempotency = "non_idempotent"

[[patch.select]]
service = "task-automation"
path_prefix = "/api/v3"
methods = ["POST", "PUT", "PATCH", "DELETE"]
risk = "high"
idempotency = "non_idempotent"
"#,
    ));
    let mut published = ids(&connector);
    published.sort_unstable();
    assert_eq!(
        published,
        vec![
            "babelforce-agent-action-on-task",
            "babelforce-agent-interaction-duration",
            "babelforce-agent-interactions",
            "babelforce-change-agent-lock",
            "babelforce-change-task-state",
            "babelforce-create-secrets",
            "babelforce-create-selection-configuration",
            "babelforce-delete-script",
            "babelforce-delete-secret-keys",
            "babelforce-delete-selection-configuration",
            "babelforce-get-script",
            "babelforce-list",
            "babelforce-list-scripts",
            "babelforce-list-secret-keys",
            "babelforce-list-secret-prefixes",
            "babelforce-manager-interrupt-on-task",
            "babelforce-patch-secrets",
            "babelforce-read-selection-configuration",
            "babelforce-submit-script",
            "babelforce-submit-task",
            "babelforce-submit-task-template",
            "babelforce-task",
            "babelforce-task-journal",
            "babelforce-task-usage",
            "babelforce-task-usage-types",
            "babelforce-tasks",
            "babelforce-testing",
            "babelforce-update-script",
            "babelforce-update-selection-configuration",
            "babelforce-update-task",
        ],
        "the derived id set is a public contract and moves only on purpose"
    );
}

// ---------------------------------------------------------------------------------------------
// C-414 · Risk and idempotency by selector, with silence refusing
// ---------------------------------------------------------------------------------------------

/// A selector states `risk` and `idempotency` for every operation it matched — 54 DELETEs as one
/// reviewable line rather than 54 blocks.
#[test]
fn a_selector_states_risk_and_idempotency_for_the_set() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["DELETE"]
risk = "destructive"
idempotency = "non_idempotent"
"#,
    ));
    assert!(!connector.operations.is_empty());
    assert!(
        connector
            .operations
            .iter()
            .all(|operation| operation.risk == Risk::Destructive
                && operation.idempotency == Idempotency::NonIdempotent),
        "the statement reaches every matched operation"
    );
}

/// **Silence on an authored write refuses the build.** It must not default to `low`, regardless of
/// the transport method carrying it.
#[test]
fn silence_on_an_authored_write_refuses() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["DELETE"]
"#,
    ));
    assert!(
        refusal.contains("deleteAgent") && refusal.contains("risk"),
        "an unstated DELETE must be refused by name: {refusal}"
    );
    assert!(
        !refusal.is_empty() && refusal.contains("authored write"),
        "the refusal names the authored direction that makes silence unacceptable: {refusal}"
    );
}

/// An authored read may leave risk and idempotency unstated: it has no vendor-state damage claim to
/// get wrong, and the values a read forces are `low` and `idempotent`. Direction itself remains an
/// explicit identity-keyed fact. The asymmetry with the test above is the whole of C-414.
#[test]
fn a_read_may_go_unstated() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
"#,
    ));
    assert!(!connector.operations.is_empty());
    assert!(
        connector
            .operations
            .iter()
            .all(|operation| operation.risk == Risk::Low
                && operation.idempotency == Idempotency::Idempotent),
        "a read takes the values it cannot get wrong"
    );
}

/// **A selector is not a bulk escape hatch around C-186.** `conditional` over a set of writes still
/// owes a stated condition per operation, and a selector cannot state one for many operations at
/// once — so the build refuses, naming each.
#[test]
fn a_bulk_conditional_still_owes_a_condition_per_operation() {
    let refusal = refuse(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["DELETE"]
risk = "destructive"
idempotency = "conditional"
"#,
    ));
    assert!(
        refusal.contains("repeatable_because") && refusal.contains("babelforce-delete-agent"),
        "a bulk `conditional` must refuse per operation: {refusal}"
    );
    assert!(
        !POINTER.contains("repeatable_because"),
        "a selector must not be able to state one condition for many operations"
    );
}

/// A per-operation block overrides a selector's risk, by the same precedence C-411 states.
#[test]
fn a_block_overrides_a_selectors_risk() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["DELETE"]
risk = "high"
idempotency = "non_idempotent"

[[patch.operations]]
service = "manager"
select = "deleteAgent"
risk = "destructive"
"#,
    ));
    assert_eq!(
        operation(&connector, "babelforce-delete-agent").risk,
        Risk::Destructive
    );
    assert_eq!(
        operation(&connector, "babelforce-delete-agent-group").risk,
        Risk::High
    );
}

// ---------------------------------------------------------------------------------------------
// `expose` by selector — C-413's field, declared in bulk
// ---------------------------------------------------------------------------------------------

/// Exposure stays defaulted-on and is stated by a selector for the set it matched. 397 catalogued
/// and callable, a curated handful reaching a model, and no line per operation to say so.
#[test]
fn a_selector_states_exposure_for_the_set() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
expose = false
"#,
    ));
    assert!(!connector.operations.is_empty());
    assert!(
        connector
            .operations
            .iter()
            .all(|operation| !operation.expose),
        "a selector's `expose` reaches every operation it matched"
    );
}

/// Silence keeps today's behaviour: an operation nobody said anything about is exposed.
#[test]
fn exposure_still_defaults_to_exposed() {
    let connector = load(&with(
        r#"
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
"#,
    ));
    assert!(connector
        .operations
        .iter()
        .all(|operation| operation.expose));
}

// ---------------------------------------------------------------------------------------------
// The scope constraint, as a number
// ---------------------------------------------------------------------------------------------

/// **The canonical surface, selected by a file a reviewer can read.**
///
/// This is what the three declarations are for, and it is the number C-417 depends on. The fixture
/// beside this file is the selection; the assertions are the scope constraint restated:
///
/// - every operation of the 397 that ingest can express, and nothing else — `POST
///   /api/v1/webhook/zendesk` is excluded by stating the prefix that holds its 30 siblings, so the
///   exclusion is a statement about what is wanted rather than a list of what is not;
/// - the five it cannot express and the four it withholds are named, so `388 + 5 + 4 = 397` is an
///   accounting rather than a shortfall nobody looked at;
/// - the nine ids `providers/babelforce.toml` ships today, unmoved, and the only nine exposed;
/// - no `internal` segment anywhere.
///
/// The line count is asserted too, because "one statement covers many operations" is a claim about
/// how much a human has to read, and an unasserted claim about size is how the boilerplate comes
/// back.
#[test]
fn the_canonical_surface_is_selected_and_the_file_stays_reviewable() {
    let definition = read("providers/babelforce.toml");
    let loaded = provider::load_with_spec("providers/babelforce.toml", &definition, &cache())
        .unwrap_or_else(|error| panic!("the canonical fixture must load:\n{error}"));
    let connector = &loaded.connector;

    assert_eq!(
        connector.operations.len(),
        REACHABLE,
        "the selection is every operation of the canonical surface that ingest can express"
    );

    // The five it cannot, named — the whole of the difference between what was selected and the
    // 397 the scope constraint states, and attributable to one cause.
    let skipped: BTreeSet<&str> = loaded
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.problem.contains("multipart/form-data"))
        .filter_map(|diagnostic| diagnostic.location.split_once(' '))
        .map(|(_, path)| path)
        .collect();
    assert_eq!(
        skipped,
        MULTIPART.into_iter().collect::<BTreeSet<_>>(),
        "the inexpressible half of the gap is exactly the multipart uploads ingest skips; if this \
         set moved, so did the accounting"
    );

    // The other half of the gap, and it is a *decision* rather than a limit: none of the four
    // credential-carrying paths is selected. Asserted by name, because each is a path a widening
    // prefix would sweep back in silently.
    let selected: BTreeSet<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.path.as_str())
        .collect();
    let reached: Vec<&str> = WITHHELD
        .into_iter()
        .filter(|path| selected.contains(path))
        .collect();
    assert!(
        reached.is_empty(),
        "the canonical selection reaches operations withheld for the credentials they carry: \
         {reached:?}. An `/oauth/*` endpoint describes how to authenticate and is never an \
         operation; `/api/v2/user/account` delivers a credential in its response body"
    );

    assert_eq!(
        connector.operations.len() + MULTIPART.len() + WITHHELD.len(),
        CANONICAL,
        "{REACHABLE} selected + {} inexpressible + {} withheld = the {CANONICAL} the scope \
         constraint names",
        MULTIPART.len(),
        WITHHELD.len()
    );

    assert!(
        !connector
            .operations
            .iter()
            .any(|operation| operation.path == "/api/v1/webhook/zendesk"),
        "the webhook receiver is not a callable operation"
    );
    assert!(
        !connector.operations.iter().any(|operation| operation
            .path
            .split('/')
            .any(|segment| segment == "internal")),
        "no operation on an `internal` path may ever be selected"
    );

    // The nine `providers/babelforce.toml` ships today. C-417 widens that file to this selection,
    // so these ids are the compatibility target and must survive it.
    for id in [
        "babelforce-agent-list",
        "babelforce-agent-get",
        "babelforce-agent-status-update",
        "babelforce-call-list",
        "babelforce-call-get",
        "babelforce-call-hangup",
        "babelforce-call-session-set",
        "babelforce-session-get",
        "babelforce-session-update",
    ] {
        let published = operation(connector, id);
        assert!(
            published.expose,
            "the curated nine stay exposed while the rest are merely callable"
        );
    }

    let unexposed = connector
        .operations
        .iter()
        .filter(|operation| !operation.expose)
        .count();
    assert_eq!(
        unexposed,
        REACHABLE - 9,
        "everything past the curated nine is catalogued and callable without reaching a model"
    );

    // **The size claim, measured.** Direction is intentionally one reviewed value per stable
    // operation identity; it therefore scales with the surface and is not selector boilerplate.
    // Count the remaining declarations, where one `[[patch.operations]]` block per operation would
    // still be north of 1,600 lines.
    let declarations = definition
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| !line.ends_with("= \"read\"") && !line.ends_with("= \"write\""))
        .count();
    assert!(
        declarations < 400,
        "selecting {REACHABLE} operations took {declarations} declaration lines; the point of a \
         selector is that this number does not scale with the operation count"
    );
}
