//! A patch may **drop** a parameter the vendor declares — C-422.
//!
//! `spec_backed_provider.rs` covers what a patch may *correct* about a parameter. This covers the
//! other half of the same statement: what it may remove, and — the part that matters — what it
//! refuses to remove.
//!
//! # Why this reads the real document rather than a fixture
//!
//! The capability was not anticipated, it was **measured**. C-416 converted babelforce from
//! hand-authored to spec-backed and found the conversion cheaper everywhere except one endpoint:
//! `listReportingCalls` declares **38 query parameters**, most of them the vendor's own aliases of
//! each other (`fromNumber` for `from`, and a whole `filters.` prefixed restatement of the set). The
//! hand-authored operation curated **14** of them. Nothing about that is reproducible against an
//! invented fixture, because the thing under test is that a real vendor's real synonym flood comes
//! back to a reviewed argument list — so these tests select out of
//! `specs/babelforce/manager-2026-07-10.openapi.yaml`, the same bytes `providers/babelforce.toml`
//! will point at.
//!
//! The provider definition here is a fixture, though, and deliberately: `providers/babelforce.toml`
//! is still hand-authored and belongs to C-416. This file compiles a definition of its own against
//! the vendored document, so the two stories cannot break each other.

use std::path::{Path, PathBuf};

use connector_spec::{provider, Connector, SpecDocument};

/// The vendored document these tests select out of, spelled as `[spec] path` spells it.
const PINNED: &str = "specs/babelforce/manager-2026-07-10.openapi.yaml";

fn document() -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PINNED);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// A spec cache holding exactly the vendored manager document.
fn cache() -> Vec<SpecDocument<'static>> {
    // Leaked for the same reason `spec_backed_provider.rs` leaks: a `SpecDocument<'static>` keeps
    // every helper below free of a lifetime, and a test process is the one place that costs nothing.
    let document: &'static str = Box::leak(document().into_boxed_str());
    vec![SpecDocument {
        path: PINNED,
        document,
    }]
}

/// The `[spec]` pointer every fixture below carries.
const POINTER: &str = "\
id = \"babelforce\"
vendor = \"Babelforce\"
base_url = \"https://services.babelforce.com\"

[spec]
path = \"specs/babelforce/manager-2026-07-10.openapi.yaml\"
";

fn with(patch: &str) -> String {
    format!("{POINTER}{patch}")
}

fn load(definition: &str) -> Connector {
    provider::load_with_spec("providers/babelforce-fixture.toml", definition, &cache())
        .unwrap_or_else(|error| panic!("this definition was expected to load: {error}"))
        .connector
}

/// The problems `definition` produces, rendered as the author would read them.
fn refuse(definition: &str) -> String {
    provider::load_with_spec("providers/babelforce-fixture.toml", definition, &cache())
        .err()
        .unwrap_or_else(|| panic!("this definition was expected not to load:\n{definition}"))
        .to_string()
}

/// The query parameter names of `id`, in the order the connector carries them.
fn query(connector: &Connector, id: &str) -> Vec<String> {
    connector
        .operation(id)
        .unwrap_or_else(|| panic!("{id} was selected"))
        .params
        .query
        .iter()
        .map(|param| param.name.clone())
        .collect()
}

/// Selecting `listReportingCalls` with no `omit`, which is where the 38 come from.
const SELECT_CALL_LIST: &str = "
[[patch.operations]]
select = \"listReportingCalls\"
direction = \"read\"
rename = \"babelforce-call-list\"
risk = \"low\"
idempotency = \"idempotent\"
";

/// The curated 14, in the document's own order — the set `providers/babelforce.toml` hand-authored.
const CURATED: [&str; 14] = [
    "page",
    "max",
    "sessionId",
    "conversationId",
    "id",
    "type",
    "fromNumber",
    "toNumber",
    "time.start",
    "time.end",
    "agentId",
    "q",
    "state",
    "finishReason",
];

// ---------------------------------------------------------------------------------------------
// Omission is explicit and never inferred
// ---------------------------------------------------------------------------------------------

/// **The "before" number, asserted rather than quoted.** A patch that says nothing about parameters
/// gets every parameter the vendor declares, all 38 of them.
///
/// This is the half of "omission is explicit" that a test can hold: no heuristic thins the argument
/// list on the author's behalf, so a document that grows a parameter upstream grows the tool, and
/// that shows up in a diff instead of being quietly absorbed. It also fixes the baseline the test
/// below measures against, so "the curated list came back" cannot be true by accident.
#[test]
fn nothing_is_dropped_unless_the_patch_says_so() {
    let connector = load(&with(SELECT_CALL_LIST));
    assert_eq!(
        query(&connector, "babelforce-call-list").len(),
        38,
        "the vendor declares 38 query parameters and the overlay must publish all of them until \
         told otherwise"
    );
}

/// **The "after" number, and the reason this story exists.** Twenty-four names in the patch turn a
/// 38-argument tool back into the 14-argument one that was hand-authored.
///
/// The dropped set is the vendor's own redundancy: `from`/`to` against the `fromNumber`/`toNumber`
/// the connector publishes, the eighteen `filters.`-prefixed restatements of parameters already
/// declared unprefixed, and four filters the curated operation never offered. None of that is
/// inferable — `filters.q` and `q` are the same filter only because the vendor's prose says so — so
/// every one of them is written down here and survives regeneration.
#[test]
fn the_curated_argument_list_comes_back_when_the_patch_names_what_to_drop() {
    let connector = load(&with(
        "
[[patch.operations]]
select = \"listReportingCalls\"
direction = \"read\"
rename = \"babelforce-call-list\"
risk = \"low\"
idempotency = \"idempotent\"
omit.query = [
  \"parentId\", \"from\", \"to\", \"domain\", \"source\", \"anonymous\",
  \"filters.sessionId\", \"filters.conversationId\", \"filters.id\", \"filters.parentId\",
  \"filters.type\", \"filters.from\", \"filters.fromNumber\", \"filters.to\",
  \"filters.toNumber\", \"filters.time.start\", \"filters.time.end\", \"filters.agentId\",
  \"filters.q\", \"filters.state\", \"filters.domain\", \"filters.source\",
  \"filters.finishReason\", \"filters.anonymous\",
]
",
    ));

    assert_eq!(query(&connector, "babelforce-call-list"), CURATED);
}

/// Omission touches the parameters and nothing else: the operation keeps the document's method,
/// path, description and response schema.
///
/// Worth stating because the implementation removes entries from a cloned `ParamSet`, and a version
/// of it that rebuilt the operation instead could drop something else on the way past without any
/// other test noticing.
#[test]
fn omitting_a_parameter_changes_only_the_parameters() {
    let whole = load(&with(SELECT_CALL_LIST));
    let narrowed = load(&with(
        "
[[patch.operations]]
select = \"listReportingCalls\"
direction = \"read\"
rename = \"babelforce-call-list\"
risk = \"low\"
idempotency = \"idempotent\"
omit.query = [\"filters.q\"]
",
    ));

    let before = whole.operation("babelforce-call-list").expect("selected");
    let after = narrowed
        .operation("babelforce-call-list")
        .expect("selected");
    assert_eq!(before.method, after.method);
    assert_eq!(before.path, after.path);
    assert_eq!(before.description, after.description);
    assert_eq!(before.response_schema, after.response_schema);
    assert_eq!(before.params.query.len(), after.params.query.len() + 1);
}

// ---------------------------------------------------------------------------------------------
// What omission refuses
// ---------------------------------------------------------------------------------------------

/// **Omitting a required parameter is refused.** `exportAgents` declares `format` required, and a
/// connector that drops it composes a request the vendor rejects — the one omission whose cost is a
/// runtime failure rather than a wide tool.
#[test]
fn omitting_a_required_parameter_is_refused() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"exportAgents\"
direction = \"read\"
rename = \"babelforce-agent-export\"
risk = \"low\"
idempotency = \"idempotent\"
omit.query = [\"format\"]
",
    ));
    assert!(rendered.contains("format"), "{rendered}");
    assert!(rendered.contains("required"), "{rendered}");
}

/// A path parameter with no exact configuration pin is refused whatever its `required` flag says,
/// because the path template keeps its placeholder: dropping `actionType` from
/// `/api/v2/actions/{actionType}/{actionName}` leaves a URL nothing can compose.
#[test]
fn omitting_a_path_parameter_is_refused() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"executeAction\"
direction = \"write\"
rename = \"babelforce-action-execute\"
risk = \"high\"
idempotency = \"non_idempotent\"
omit.path = [\"actionType\"]
",
    ));
    assert!(rendered.contains("actionType"), "{rendered}");
    assert!(rendered.contains("{actionType}"), "{rendered}");
}

/// **Omitting what the document does not declare is a loud error**, exactly as an unmatched
/// correction already is. Same rot, same treatment: the vendor renames a parameter, the line that
/// used to drop it stops applying, and the argument the connector spent a story removing comes back
/// into the tool with the build still green.
#[test]
fn omitting_a_parameter_the_document_does_not_declare_is_refused() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"listReportingCalls\"
direction = \"read\"
rename = \"babelforce-call-list\"
risk = \"low\"
idempotency = \"idempotent\"
omit.query = [\"filters.callerNumber\"]
",
    ));
    assert!(rendered.contains("filters.callerNumber"), "{rendered}");
    assert!(rendered.contains("listReportingCalls"), "{rendered}");
    assert!(rendered.contains("does not declare"), "{rendered}");
}

/// The position is half the identity, so a name that exists in another group does not match. The
/// vendor may bind one name in two places and the overlay must not guess which one an author meant.
#[test]
fn omitting_a_parameter_from_the_wrong_position_is_refused() {
    let rendered = refuse(&with(
        "
[[patch.operations]]
select = \"listReportingCalls\"
direction = \"read\"
rename = \"babelforce-call-list\"
risk = \"low\"
idempotency = \"idempotent\"
omit.header = [\"page\"]
",
    ));
    assert!(rendered.contains("page"), "{rendered}");
    assert!(rendered.contains("Header"), "{rendered}");
}

/// Corrections are applied **before** omissions, so requiredness is judged as the connector states
/// it rather than as the vendor guessed it.
///
/// That ordering is what makes the required refusal usable instead of merely strict: a vendor that
/// marks a parameter required when it is not (`provider-operation-inventory.md` §6.4 records
/// Freshdesk doing the inverse) would otherwise pin an argument into every tool with no way out.
/// The author corrects the flag, which is a reviewable statement in its own right, and only then may
/// drop it.
#[test]
fn a_correction_is_applied_before_the_omission_that_depends_on_it() {
    let connector = load(&with(
        "
[[patch.operations]]
select = \"exportAgents\"
direction = \"read\"
rename = \"babelforce-agent-export\"
risk = \"low\"
idempotency = \"idempotent\"
omit.query = [\"format\"]

[[patch.operations.params]]
name = \"format\"
position = \"query\"
required = false
",
    ));
    assert!(
        query(&connector, "babelforce-agent-export").is_empty(),
        "the corrected parameter was the only one, so the omission empties the group"
    );
}
