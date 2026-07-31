//! **`response_schema` coverage, measured over the shipped catalogue and ratcheted.**
//!
//! `docs/designs/member-io-schemas.md` measured 16 of 97 operations carrying a response shape and
//! drew the conclusion this file exists to enforce: *coverage that nothing watches only ever goes
//! down*. Not because anyone removes a schema — because a new connector ships without response
//! shapes, the denominator grows, and the ratio falls with nobody noticing. A number in a design
//! document is a snapshot; a floor is a ratchet.
//!
//! So two figures are recorded, and both are floors:
//!
//! - [`COVERED_FLOOR`] — the absolute count. Deleting a schema fails here.
//! - [`RATIO_FLOOR_PERCENT`] — the share of all shipped operations. **Adding operations without
//!   response shapes fails here**, which is the regression the count alone cannot see and the one
//!   that actually happened between the design's measurement and C-126.
//!
//! Both may be *raised* freely — that is the ratchet turning. Lowering either is a deliberate act
//! that belongs in a story with a reason, not a quiet edit to make a build green.
//!
//! The third test is what keeps the measure honest. A coverage count is trivially gameable by
//! declaring `{}` or `{"type": "object"}` on everything: both satisfy "declares a response schema"
//! and tell a consumer nothing at all, so a permissive placeholder is worse than absence — it is
//! indistinguishable from a real declaration. **Absence stays absence**, and this file refuses the
//! placeholder rather than trusting an author to.
//!
//! What is *not* asserted here, deliberately: that any particular operation carries a schema. Some
//! vendor responses are genuinely unspecified or vary by account (babelforce's manager document is
//! not vendored, and §1.3 of `docs/designs/provider-operation-inventory.md` says why it cannot be),
//! and a schema nobody can rely on is not an improvement on none. The floor measures the aggregate
//! and leaves the per-operation judgement to the provider file, where it is reviewable.
//!
//! Read the measurement in `response_schema`'s own terms: it describes **what the vendor sends**, not
//! what a caller of the emitted `op` receives. `http.request` returns one flat string today, so the
//! effective output is `String` for every operation without exception — the distinction C-127 owns,
//! and the reason nothing here calls this an output schema.

use std::path::{Path, PathBuf};

use connector_spec::JsonSchema;

/// Operations carrying a `response_schema`, as measured by [`coverage`]. **Raise this when coverage
/// rises; do not lower it to make a build green.**
///
/// C-126 measured **29 of 110** on entry and left **92 of 110**. The entry figure is itself the
/// ratchet's argument: the design recorded 16 of 97, then stripe (8 of 8) and notion (5 of 5) landed,
/// and nothing recorded what that did to the ratio in either direction.
///
/// The eighteen operations deliberately left absent, per provider: babelforce 9 (no public reference,
/// and the one authoritative document cannot be vendored — `providers/babelforce.toml` records why),
/// fly 4 (the vendor's own spec declares the lifecycle writes' `200` with no body schema), google 2
/// (Drive's default field projection is undocumented), jira 1 and zoom 1 (`204`, no body at all),
/// hubspot 1 (its `PATCH` reference renders no response section). Every one of them says so in its
/// provider file, next to the operation.
///
/// **Raised 193 → 220 by the 2026-07-31 wave, and the reason it had to be raised is the mechanism
/// working.** Statuspage (C-181, 5 of 5), Okta (C-161, 4 of 5 — its deactivation answers with an
/// empty body and declares no schema rather than a permissive placeholder) and PagerDuty (C-162,
/// 6 of 6) each fitted inside the slack **alone**, so each correctly reported eight red tests and
/// left this file untouched. Their *accumulation* crossed it: 220 of 248 against a floor of 193,
/// where the slack is 24. That is exactly the per-wave-not-per-story case `AGENTS.md` records, and
/// it is why this constant is coordinator-owned — three concurrent provider stories that each
/// raised it would collide on one line.
const COVERED_FLOOR: usize = 220;

/// The same floor as a share of every shipped operation, in whole percent. This is the half that
/// notices a connector arriving with no response shapes at all.
///
/// 92 of 110 is 83%, and the floor is set one point under the measurement deliberately: a single
/// honest absence — one operation whose vendor documents no body — should not turn an unrelated
/// provider story red on arrival, while a connector landing with nothing still does. There is no room
/// in one point for a whole provider.
///
/// **Raised 82 → 87 on 2026-07-31, because it had drifted out of its own design.** 82 was one point
/// under 83 when the measurement was 92 of 110. The measurement is now 220 of 248 — **88.7%** — so
/// the gap had quietly become *six* points, which at 248 operations is room for roughly sixteen
/// unschematized operations: a whole provider landing with nothing, which is precisely the arrival
/// this constant exists to catch. It was doing the archaeology its sibling's ratchet was built to
/// prevent.
///
/// **And that is the finding: this constant has no ratchet.** `the_recorded_floor_is_the_measured_
/// figure` turns [`COVERED_FLOOR`] both ways; nothing turns this one, so it can only drift. Wiring
/// the same two-way check here — or deriving this from `COVERED_FLOOR` rather than storing it twice
/// — is worth a story. Until then it is raised by hand, which is the mechanism that just failed.
const RATIO_FLOOR_PERCENT: usize = 87;

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// Every shipped provider, read from `providers/` rather than listed here (C-54): a list would drift
/// in exactly one direction — a provider lands and the measurement silently stops covering it, which
/// is the failure this whole file is about.
fn shipped() -> Vec<String> {
    let dir = providers_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").file_name())
        .filter_map(|name| {
            name.to_string_lossy()
                .strip_suffix(".toml")
                .map(str::to_string)
        })
        .collect();
    names.sort();
    assert!(
        !names.is_empty(),
        "{} holds no provider definitions, so the coverage below would be a vacuous 0 of 0",
        dir.display()
    );
    names
}

fn load(name: &str) -> connector_spec::Connector {
    let path = providers_dir().join(format!("{name}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    connector_spec::provider::load(&format!("providers/{name}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{name}.toml does not load: {error}"))
        .connector
}

/// One provider's contribution to the measurement.
struct Tally {
    provider: String,
    operations: usize,
    covered: usize,
}

/// The measurement itself: every shipped operation, and how many declare a response shape.
///
/// It reads `providers/*.toml` through the real loader rather than `web/public/catalog.json`, because
/// the catalogue is a whole-catalogue artifact a scoped build deliberately leaves stale — measuring
/// there would report the last full build's figure and call it today's.
fn coverage() -> Vec<Tally> {
    shipped()
        .into_iter()
        .map(|provider| {
            let connector = load(&provider);
            Tally {
                operations: connector.operations.len(),
                covered: connector
                    .operations
                    .iter()
                    .filter(|operation| operation.response_schema.is_some())
                    .count(),
                provider,
            }
        })
        .collect()
}

/// A per-provider table, so a failure says which connector moved rather than only that the total did.
fn table(tallies: &[Tally]) -> String {
    tallies
        .iter()
        .map(|tally| {
            format!(
                "  {:<12} {:>3} / {:<3}\n",
                tally.provider, tally.covered, tally.operations
            )
        })
        .collect()
}

/// **The ratchet.** Coverage may rise; it may not fall.
#[test]
fn response_schema_coverage_does_not_fall_below_its_floor() {
    let tallies = coverage();
    let operations: usize = tallies.iter().map(|tally| tally.operations).sum();
    let covered: usize = tallies.iter().map(|tally| tally.covered).sum();
    let percent = covered * 100 / operations;

    // Printed on every run, pass or fail: the figure is the by-product this test exists to keep
    // visible, and `cargo test -- --nocapture` is where a reviewer reads it.
    println!(
        "response_schema coverage: {covered} / {operations} ({percent}%)\n{}",
        table(&tallies)
    );

    assert!(
        covered >= COVERED_FLOOR,
        "response_schema coverage fell to {covered} of {operations}; the recorded floor is \
         {COVERED_FLOOR}. Coverage may rise freely — lowering the floor is a deliberate decision \
         that belongs in a story, not a fix for a red build.\n{}",
        table(&tallies)
    );
    assert!(
        percent >= RATIO_FLOOR_PERCENT,
        "response_schema coverage is {covered} of {operations} ({percent}%), below the recorded \
         floor of {RATIO_FLOOR_PERCENT}%. The count did not fall, so operations were added without \
         response shapes — declare them, or lower this floor deliberately and say why.\n{}",
        table(&tallies)
    );
}

/// **The other direction of the ratchet: a floor nobody raised is a floor that stopped measuring.**
///
/// Without this, [`COVERED_FLOOR`] could sit at its entry value of 29 forever while the catalogue
/// improved, and the "current figure" in the header would quietly become archaeology. So coverage is
/// allowed to run ahead of the floor by up to a tenth of the catalogue — enough that a provider story
/// which adds shapes need not touch this file — and beyond that the floor has to be moved up in the
/// same commit that earned it.
#[test]
fn the_recorded_floor_is_the_measured_figure() {
    let tallies = coverage();
    let operations: usize = tallies.iter().map(|tally| tally.operations).sum();
    let covered: usize = tallies.iter().map(|tally| tally.covered).sum();

    assert!(
        covered <= COVERED_FLOOR + operations / 10,
        "coverage is {covered} of {operations} but the floor still records {COVERED_FLOOR}. Raise \
         it in the same commit that raised coverage, so the ratchet only turns one way.\n{}",
        table(&tallies)
    );
}

/// **Absence stays absence.** A declared response schema has to say something.
///
/// The two shapes refused are the ones that pass a coverage count while carrying no information:
/// `{}` — which admits every JSON document — and an `object` schema with no `properties`, no
/// `items`, no `required` and no `$ref`, which is the same statement dressed as a type. An operation
/// whose response shape is genuinely unknown declares **nothing**, and that is a reviewable, honest
/// answer; a placeholder is neither.
#[test]
fn no_operation_publishes_a_permissive_response_schema() {
    for provider in shipped() {
        let connector = load(&provider);
        for operation in &connector.operations {
            let Some(schema) = &operation.response_schema else {
                continue;
            };
            assert!(
                !is_permissive(schema),
                "providers/{provider}.toml: `{}` declares a response schema that constrains \
                 nothing: {schema}. It counts towards coverage and tells a consumer no more than \
                 absence would — declare the shape the vendor documents, or declare nothing.",
                operation.id
            );
        }
    }
}

/// Whether a schema admits every document it could be checked against — an empty object, or a type
/// with no stated members.
fn is_permissive(schema: &JsonSchema) -> bool {
    let Some(object) = schema.as_object() else {
        // A bare `true` is the JSON Schema spelling of "anything".
        return schema.as_bool() == Some(true);
    };
    if object.is_empty() {
        return true;
    }
    const INFORMATIVE: [&str; 8] = [
        "properties",
        "items",
        "required",
        "$ref",
        "oneOf",
        "anyOf",
        "allOf",
        "const",
    ];
    !INFORMATIVE.iter().any(|key| object.contains_key(*key))
}
