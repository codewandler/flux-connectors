# Design: generated connector tests — what can be derived, and what must never be

**Status:** proposed · **Pillar:** Build · **Stories:** C-423 · C-424 · C-425
**Epic:** `generated-connector-tests`

## Why

`crates/connector-flux/tests/` holds **52 `*_connector.rs` files totalling 22,455 lines** — roughly
one per shipped provider, averaging ~430 lines. Every new connector adds one by hand. The question
this epic exists to answer is whether that is boilerplate a generator should write, and the honest
answer available before any work is: **partly, and the interesting part is not the part you would
generate.**

Measured across those 52 files:

| Restatement | Files declaring it |
|---|---|
| `const PROVIDER` | 37 |
| `const TOKEN_ENV` / a `_ENV: &str` | 36 |
| `const OPERATIONS` | 31 |
| `const CREDENTIAL` | 29 |
| `const BASE_URL` | 26 |

Every one of those is a second spelling of something `providers/<name>.toml` already states. That is
duplication, and duplication has a cheaper fix than generation.

**The counter-example is the reason this epic must start with a measurement.**
`crates/connector-flux/tests/slack_connector.rs` asserts one thing — that Slack **declares no query
parameter at all** — and spends its header explaining why: Slack's ids are opaque strings, the
emitter interpolates query values with no percent-encoding (C-30), so a read expressed as `GET
…?channel=…` would ship the same defect `zendesk-ticket-search` carries. It ends: *"nothing else in
the repository would fail if someone converted a read to a GET, and the resulting connector would
look tidier and be broken."*

No generator produces that. A generator that replaced it would delete the only thing standing between
a tidy-looking change and a broken connector.

So the two populations are different in kind, not degree, and the epic's whole value is telling them
apart before writing a line of generator.

## Approach

**Three questions, in order. The first is a spike and may end the epic.**

### 1 · What do the 52 files actually assert? (C-423)

Classify every assertion into one of three buckets:

- **Restates the TOML** — an op-id list, a credential name, an env var, a base URL. The provider file
  is the source of truth and the test is a copy that can disagree with it.
- **Already covered fleet-wide** — `crates/connector-flux/tests/shipped_modules.rs` enumerates
  `providers/*.toml` **from disk** (C-54) and asserts every operation of every shipped provider emits
  Flux that parses, analyzes and is canonical. Anything a per-connector file says that this already
  says is dead weight.
- **A specific, reasoned claim** — Slack's no-query property. Hand-written, load-bearing, stays.

The output is a count per bucket. That number decides everything downstream, and it is cheap to get.

### 2 · The likely answer for bucket one is deletion, not generation (C-424)

If a test restates the provider file, generating it from the provider file makes it **tautological**:
derive the expected value from the IR, assert the artifact matches, and you have asserted that the
generator is the generator. `flux-connectors diff` already checks all 557 artifacts byte-for-byte
against exactly that derivation — a generated test asserting the same thing adds runtime and no
information.

The fleet-wide pattern already in the repo is the better shape: **one test that reads
`providers/*.toml` from disk and asserts a property of every connector**, which cannot drift because
there is no second copy. `shipped_modules.rs` and `shipped_providers.rs` are the precedent. Extending
that set and deleting what it subsumes is a smaller change than a generator and strictly stronger —
it fails when a *new* provider violates the property, which a generated per-connector file only does
after someone regenerates.

**This is the design's central claim: for the mechanical bucket, the answer is probably "delete it
into a fleet-wide assertion", not "generate it".**

### 3 · What generation could add that nothing has today (C-425)

The genuinely new capability is not reducing boilerplate — it is a **test oracle this repo has never
had**: the vendored documents carry `example` blocks, and since C-4 the connectors carry
`response_schema` derived from those same documents. Nothing checks that the vendor's own example
satisfies the vendor's own declared schema.

That is a real, generatable, per-operation test with a real failure mode — it catches a vendor
document that contradicts itself, and it catches an ingest that resolved a `$ref` wrongly. babelforce
alone would supply 352 such cases from the manager document, and the check is derived from two
independent things (example and schema) rather than from one thing twice, so it is not tautological.

## Alternatives considered

- **Generate a per-connector test file per provider, committed.** The obvious reading of the request,
  and rejected as the default: it multiplies the tautology problem by 53 and puts 22,000 generated
  lines into review. If C-423 finds a large bucket-three population it may come back.
- **Generate tests at build time, uncommitted.** Worse: invisible in review, and the repo's whole
  posture is that generated artifacts are committed and read as a diff (`connector-pipeline.md`,
  "Generation is an explicit, reviewed step").
- **Property-based tests over the IR.** Attractive and orthogonal — it tests the *compiler*, not the
  connectors, so it belongs to a different epic if it is wanted at all.
- **Delete the per-connector files wholesale.** Refused: it would throw away the bucket-three claims,
  which are the ones that caught real defects.

## Risks & open questions

- **The tautology trap is the main risk and it is easy to walk into.** Any generated assertion whose
  expected value comes from the same IR that produced the artifact tests nothing. Every story here
  must state what independent thing its assertion is checked against.
- **Deleting a test is irreversible in review terms** — the reasoning goes with it. C-424 must move
  claims into fleet-wide tests, not drop them, and say what it moved.
- **The bucket-three population may be larger than the header constants suggest.** The measurement
  might show that most of the 22,455 lines are reasoned claims, in which case the correct outcome of
  this epic is *"not doable, and here is the evidence"* — which is a successful outcome and must be
  recordable as one.
- Open: whether a vendor `example` failing its own schema should fail the build or produce a
  diagnostic. Probably a diagnostic — it is the vendor's defect, not ours, and C-4 already
  established that grade of failure.

## Acceptance / done

The epic is done when the question is **answered with a number**: how many of the 22,455 lines
restate the provider file, how many are already covered fleet-wide, and how many are reasoned claims
that must stay. Then either the mechanical bucket is gone — folded into fleet-wide assertions that
cannot drift — or the measurement says it was never the boilerplate it looked like, and that is
written down so nobody re-opens it on a hunch.
