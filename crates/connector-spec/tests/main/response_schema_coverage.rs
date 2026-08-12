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
//! - [`COVERED_FLOOR`] — the count of operations that carry one. Deleting a schema fails here.
//! - [`ABSENCE_CEILING`] — the count of operations that do not. **Adding operations without
//!   response shapes fails here**, which is the regression the count alone cannot see and the one
//!   that actually happened between the design's measurement and C-126.
//!
//! They bound the two halves of one measurement, and neither bounds the other: a wave can raise the
//! covered count and the absent count in the same commit. `COVERED_FLOOR` may be *raised* freely and
//! `ABSENCE_CEILING` *lowered* freely — that is the ratchet turning. Moving either the other way is
//! a deliberate act that belongs in a story with a reason, not a quiet edit to make a build green.
//!
//! **Both are held to the measurement in both directions** (C-196). A bound nobody ever has to move
//! is a bound that has stopped describing anything, and it stops silently: the assertion goes on
//! passing while the gap it allows grows wide enough to drive a whole connector through.
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

use crate::shipped_provider;

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
///
/// **Raised 220 → 250 by the 2026-07-31 provider wave, the same mechanism a second time and at
/// larger scale.** Bitbucket (7 of 7), Mailchimp (7 of 7), Klaviyo (5 of 5), Supabase (2 of 3) and
/// Resend (4 of 4) each fitted inside the slack **alone** — every one of them checked this test and
/// correctly reported it green, left this file untouched, and reported the eight staleness failures
/// instead. Their accumulation crossed it: 250 of 281 against a floor of 220, where the slack is 28.
///
/// Two of them predicted it in their handoffs before it happened, which is the part worth keeping:
/// the per-story signal is green by construction, so an implementor cannot see a per-wave failure
/// coming. Giving them one is filed as its own story rather than fixed by lowering anything here.
// Raised 250 -> 277 at C-416's integration. babelforce became the first spec-backed provider and
// went 0/9 -> 9/9 response schemas, because the vendor's document publishes a 2xx schema for 352 of
// its 356 operations. Measured coverage is 277 of 299; the ratchet turns in the direction it is
// allowed to turn, and this records the new floor rather than leaving 27 operations of slack in it.
// Raised 606 -> 715 at C-486 integration. Asterisk ARI adds 50 source-declared response shapes and
// the accumulated spec-backed catalogue now measures 715 of 841; recording the measured figure
// keeps the upward ratchet honest.
// C-30 withholds twelve Asterisk operations whose array query serialization is undeclared. Nine
// carried response shapes, leaving 706 covered operations; the floor stays one below that measured
// count so one honest documented absence can still land without defeating the arrival guard.
const COVERED_FLOOR: usize = 705;

/// The other half of the same measurement: operations that ship **without** a response shape. This
/// is the half that notices a connector arriving with no response shapes at all.
///
/// It replaces `RATIO_FLOOR_PERCENT`, which guarded the same arrival as a share of the catalogue and
/// stopped being able to ([C-196](../../../docs/stories/C-196-the-ratio-floor-has-no-ratchet.md)).
/// Two separate failures, both measured at 268 of 299:
///
/// 1. **It had no ratchet, so it could only drift.** `the_recorded_floor_is_the_measured_figure`
///    turns [`COVERED_FLOOR`] both ways; nothing turned that one. It was moved by hand twice —
///    82 → 87 → 88 — each time *after* somebody noticed, which is exactly the archaeology its
///    sibling's second direction exists to prevent.
/// 2. **A percent was too coarse an instrument for what it guarded, and grew coarser as the
///    catalogue grew.** One point of 110 operations is one operation; one point of 299 is three. At
///    a floor of 88, **five** operations could arrive carrying nothing before the guard fired — and
///    **27 of the 53 shipped connectors are five operations or fewer**. Over half the catalogue
///    could have landed with no response shapes at all and passed.
///
/// The second is why this is not a percent with a ratchet bolted on. *No* whole-percent value both
/// admits one honest absence and refuses a three-operation connector at this catalogue size: the
/// unit was the defect, not the number. Counting absences directly makes the guard's resolution one
/// operation, and keeps it there whatever the catalogue grows to.
///
/// Deriving a ratio from [`COVERED_FLOOR`] was weighed first and rejected, because it deletes the
/// guard along with the constant. `COVERED_FLOOR * 100 / operations` puts the same denominator on
/// both sides of the comparison, so it reduces to `covered >= COVERED_FLOOR` — the check that
/// already exists — and the arrival this constant is for passes it: a nine-operation connector
/// landing with nothing leaves `covered` untouched at 268, and 268 of 308 clears a derived floor of
/// 81 comfortably. Two constants stand here because they bound two quantities that move
/// independently. Covered can rise while absent rises too, in the same commit, and neither figure
/// can be computed from the other.
///
/// **31 operations across 18 providers ship without a shape today, and they are not a defect.**
/// babelforce (0 of 9) and fly (4) are vendor-wide gaps their provider files explain; datadog and
/// google contribute 2 each; the remaining 14 are single operations, each recorded next to the
/// operation in its own provider file. [`COVERED_FLOOR`]'s doc enumerates *eighteen* — that is the
/// figure from the 110-operation era, kept as the record of what C-126 measured, not a count of
/// today's.
// Lowered 33 -> 24 at C-416's integration, the same event and the same cause: absence fell from 31
// to 22 of 299 when babelforce's nine gained the schemas its document already published. 24 is the
// value that satisfies both directions of the ratchet at the measured figure.
// Raised 69 -> 127 at C-486 integration. Asterisk's first-party descriptions honestly leave 58 of
// its 108 REST responses without a constraining schema, taking the catalogue to 126 absences. The
// extra one-operation allowance preserves the guard's stated rule while a two-operation connector
// arriving wholly unschematized still fails.
// The same C-30 deferral leaves 123 measured absences. The ceiling stays one above that figure so
// one honest absence remains admissible while the smallest shipped two-operation connector arriving
// with no response shapes still fails.
const ABSENCE_CEILING: usize = 124;

/// How far [`ABSENCE_CEILING`] may sit above the measured absence. This is the guard's resolution,
/// and the only number in this file that was chosen rather than read off the catalogue, so it is the
/// one that owes an argument.
///
/// It is bounded from above by the smallest shipped connector. supabase ships **3** operations, so a
/// slack of 3 would let a connector exactly that size land carrying nothing and stay green, which is
/// the one arrival [`ABSENCE_CEILING`] exists to catch. It is bounded from below by the design this
/// file has always stated: a single honest absence — one operation whose vendor documents no body —
/// must not turn an unrelated provider story red on arrival, so it is at least 1.
///
/// That leaves 1 or 2, and 2 is the measured answer. datadog (2 of 4) and google (6 of 8) each
/// arrived carrying exactly two operations whose vendors document no response body; a slack of 1
/// would have turned both of those stories red on arrival for doing nothing wrong.
///
/// A story landing **three or more** honest absences is therefore red on arrival, and deliberately
/// so — that is a claim about the catalogue large enough to be worth a sentence in a story. It
/// reports this test alongside the ninth staleness check and stops; the coordinator moves the
/// ceiling at integration, in the commit that earned it. Same per-wave rhythm [`COVERED_FLOOR`]
/// already has, and the same reason both constants are fenced to the coordinator: three concurrent
/// provider stories that each moved one would collide on a single line.
const ABSENCE_SLACK: usize = 2;

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

/// Through C-421's shared seam, because this measurement reads the **shipped** definitions.
///
/// A spec-backed provider's operations are a function of its vendored document as well as of its
/// file, so plain `provider::load` cannot answer for one — it refuses rather than under-reporting,
/// which is how this call site was found. Coverage measured through the pure loader would have
/// counted babelforce's nine as absent forever.
fn load(name: &str) -> connector_spec::Connector {
    shipped_provider::connector(name)
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
        absence_is_within_bounds(operations, covered),
        "{} of {operations} operations now ship without a response shape, above the recorded \
         ceiling of {ABSENCE_CEILING}. The covered count did not fall, so operations were added \
         without response shapes — declare them, or raise this ceiling deliberately and say why.\n{}",
        operations - covered,
        table(&tallies)
    );
}

/// **The guard, as a function rather than an inline assertion**, so it can be asked about arrivals
/// that have not happened yet — which is the only way to test that a bound still bounds anything.
fn absence_is_within_bounds(operations: usize, covered: usize) -> bool {
    operations - covered <= ABSENCE_CEILING
}

/// **The stated design, asserted rather than described.** Both halves, in the file's own words: one
/// honest absence must not turn an unrelated provider story red on arrival, and a connector landing
/// with nothing at all still must.
#[test]
fn a_connector_arriving_with_no_response_shapes_is_caught() {
    let tallies = coverage();
    let operations: usize = tallies.iter().map(|tally| tally.operations).sum();
    let covered: usize = tallies.iter().map(|tally| tally.covered).sum();
    let smallest = tallies
        .iter()
        .map(|tally| tally.operations)
        .min()
        .expect("the catalogue ships at least one provider");

    assert!(
        absence_is_within_bounds(operations + 1, covered),
        "one operation whose vendor documents no body turns the guard red at {covered} of \
         {operations}. The floor is meant to leave room for a single honest absence.\n{}",
        table(&tallies)
    );
    assert!(
        !absence_is_within_bounds(operations + smallest, covered),
        "a connector the size of the smallest already shipped ({smallest} operations) could land \
         carrying no response shapes at all — {covered} of {} — and this guard would stay green. \
         That arrival is the one thing it exists to catch.\n{}",
        operations + smallest,
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

/// **The same direction, for the ceiling — and the whole of C-196.**
///
/// `RATIO_FLOOR_PERCENT` had only the forward half. It sat wherever it was last set by hand while
/// the catalogue moved underneath it, went on passing the entire time, and the gap it allowed grew
/// from one operation to five without a single test noticing. A bound that never has to move is
/// indistinguishable from a bound that has stopped measuring, and this is the test that tells them
/// apart: every absence resolved has to be given back, in the commit that resolved it.
#[test]
fn the_recorded_ceiling_is_the_measured_absence() {
    let tallies = coverage();
    let operations: usize = tallies.iter().map(|tally| tally.operations).sum();
    let covered: usize = tallies.iter().map(|tally| tally.covered).sum();
    let absent = operations - covered;

    assert!(
        ABSENCE_CEILING <= absent + ABSENCE_SLACK,
        "{absent} of {operations} operations ship without a response shape, but the ceiling still \
         records {ABSENCE_CEILING}. Absences were resolved and the ceiling kept the room they \
         freed, which is room for a connector to arrive carrying nothing. Lower it in the same \
         commit that earned it, so the ratchet only turns one way.\n{}",
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
///
/// **`connector_spec::constrains_nothing`, not a copy of it** (C-417). This was a local twenty-line
/// predicate until ingest needed the same judgement: a vendor document that publishes
/// `{"type": "object"}` for its deletes must not have that laundered into coverage, and 24 of
/// babelforce's do. Two copies of one rule with only one of them enforced is the defect this
/// repository files stories about, so the rule moved into the library and this reads it — which
/// also means a keyword added to the informative list tightens the gate and the ingest refusal
/// together, in one edit.
fn is_permissive(schema: &JsonSchema) -> bool {
    connector_spec::constrains_nothing(schema)
}
