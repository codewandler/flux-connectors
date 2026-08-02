---
id: C-452
title: "The pack cannot evaluate an array body, so the first indexed wire path turns its gate red"
pillar: Bridge
status: ready
priority: 2
epic: tool-pack
areas: [connector-pack]
note: "found by the C-185 review. The emitter can now build an array body; `eval` has no Node::List arm and refuses at request.rs:1330. Nothing in the catalogue uses one yet, so the gate is green TODAY — the first provider to write `wire = \"x[0].y\"` turns it red"
---

# The pack cannot evaluate an array body, so the first indexed wire path turns its gate red

## Goal

Give `connector_pack`'s request evaluator a `Node::List` arm, so an operation whose body contains an
array can be turned into a real HTTP request rather than refused.

## The finding

From the independent review of [C-185](C-185-body-arrays.md), verified here:

- `crates/connector-pack/src/request.rs:1330` refuses with *"its body computes {}, which this pack
  does not evaluate"*, and `:1550` renders `Node::List { .. }` as `"a list"`.
- `grep -rn 'wire = "[^"]*\[' providers/` → **no hits**. Nothing in the catalogue declares an indexed
  path, so **the gate is green today and this is not a regression.**

C-185 shipped the emitter half: `BodyNode::Elements` lowers to `Node::List` and the emitted Flux is
correct. The pack is the other half of the same capability and did not move.

## Why it is priority 2 rather than a note

**It is a latent tripwire on a capability three other pieces of work are waiting for.** The moment a
provider writes `wire = "personalizations[0].to[0].email"` — which is the entire point of C-185 — that
provider's operation stops being callable through the pack, and the failure surfaces as a whole-catalogue
gate going red rather than as one operation refusing.

Three dependents make that concrete:

- **`providers/sendgrid.toml`** is the vendor C-185 was written for; its send is still withheld.
- **`providers/anthropic.toml`** withheld the Admin API's mutating surface partly on *"`BodyNode` never
  builds an array"*. That premise is now false and the file has been corrected to cite this story
  instead — so this is what actually gates Anthropic writes.
- **The `anthropic-managed-agents` epic** cites the same array gap.

## Acceptance

- [ ] `eval` (`crates/connector-pack/src/request.rs`) handles `Node::List`, producing a JSON array in
      the evaluated body.
- [ ] **Failing-first:** a test that declares an operation with an indexed `wire` path and asserts the
      *evaluated request body* carries a real JSON array — red before the arm exists, with the refusal
      message quoted.
- [ ] The refusal at `:1330` still fires for every node kind that genuinely cannot be evaluated. This
      story removes one arm from that set, not the set.
- [ ] A round trip: the same operation emits Flux (C-185's path) **and** evaluates through the pack,
      and the two agree on the body. The two halves disagreeing silently is the failure mode worth a
      test.
- [ ] The gate is green and the build stays a fixed point.

## Progress
- (not started)

## Notes
- Do not close this by declaring an indexed path in a provider file to prove it works — that ships a
  catalogue change on the back of a pack fix. Use a fixture.
- Two smaller review findings, recorded here rather than given their own stories:
  **(a)** eight files still assert in prose that `BodyNode` has no array variant, which is now false —
  `crates/connector-flux/tests/{confluence,contentful,jira,miro,postmark,webflow}_connector.rs`,
  `providers/jira.toml:36`, `providers/contentful.toml:72`. All are doc comments; the tests beneath
  them still assert true things. `providers/anthropic.toml` was the sharpest and is corrected.
  **(b)** `body_arrays.rs::a_caller_supplied_list_of_objects_is_still_not_decomposable` asserts the
  scope boundary by grepping emitted text for absent `each `/`repeat `, so it would pass on any
  emitter that spelled iteration differently. The boundary is sound *structurally* — the op path
  constructs no `Each`/`Repeat`/`Expr`/`Jq` node at all — but that test is not what holds it.
- Also open, and deliberately not folded in: no test runs `format_cst::format_module` over a whole
  committed `connectors/*.flux`, so module-level canonicality with an array is asserted only
  per-operation. Unreachable until a provider ships one — worth a whole-file fixed-point check at that
  point.
