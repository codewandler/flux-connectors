# Design: generated connector tests — what can be derived, and what must never be

**Status:** proposed · **Pillar:** Build · **Stories:** C-423 · C-424 · C-425
**Epic:** `generated-connector-tests`

## Why

> **C-423 measured this. The answer is a negative: do not generate.** The numbers below replace the
> estimate this section originally carried. Everything from "The measurement" down is measured at
> `e9ece54`; the paragraphs above it are the premise that measurement tested.

`crates/connector-flux/tests/` holds **52 `*_connector.rs` files totalling 22,595 lines** — roughly
one per shipped provider, averaging ~434 lines. (The epic was opened against a hand-typed 22,455;
the measured figure at `e9ece54` is 22,595.) Every new connector adds one by hand. The question this
epic exists to answer is whether that is boilerplate a generator should write.

The five restatement counts that motivated the epic are all **exactly right**, and were re-verified:

| Restatement | Files declaring it | Verified |
|---|---|---|
| `const PROVIDER` | 37 | ✓ |
| `const TOKEN_ENV` / a `_ENV: &str` | 36 | ✓ |
| `const OPERATIONS` | 31 | ✓ |
| `const CREDENTIAL` | 29 | ✓ |
| `const BASE_URL` | 26 | ✓ |

**And they are not the corpus.** All 277 `const` declarations in all 52 files together occupy
**656 lines — 2.9%** of the 22,595. The duplication the epic was opened over is real, cheap, and
three per cent of the thing it was proposed as a reason to rewrite.

### The measurement (C-423)

Every assertion site in the 52 files was extracted mechanically (2,352 sites: 1,774 `assert!` /
`assert_eq!` / `assert_ne!`, plus 578 `.expect` / `.unwrap_or_else` / `panic!` guards) and classified.

**Assertions**, over the 1,774 hard assertions:

| Bucket | Assertions | Share |
|---|---|---|
| **(a) restates `providers/<name>.toml`** | 636 | **35.9%** |
| **(b) already covered fleet-wide** | 209 | **11.8%** |
| **(c) a specific reasoned claim** | 929 | **52.4%** |

**Lines**, over all 22,595:

| Bucket | Lines | Share |
|---|---|---|
| (a) | 4,348 | 19.2% |
| (b) | 1,571 | 7.0% |
| **(c)** | **11,290** | **50.0%** |
| shared preamble (module doc, imports, consts, load helper) | 5,386 | 23.8% |

Counting only the 17,209 lines inside test functions: **(a) 25.3% · (b) 9.1% · (c) 65.6%.**

Three independently-built classifiers were run and hand-audited against 60 randomly sampled
assertions labelled by reading. The reported one agrees with the hand labels on **83%** of the
sample with *balanced* errors (5 that should be (a), 4 that should be (c)); the two rejected
variants scored 85% and 82% but with errors running 8–10 deep in a single direction. Across all
three, bucket (c) brackets at **50–63%** of assertions and bucket (a) at **26–39%**. The conclusion
does not turn on which is used.

### Bucket (c) dominates, and it is dominant in kind as well as in count

- **369 of the 384 distinct test-function names appear in exactly one file.** Only 15 names recur at
  all, and the most-repeated (`the_curated_operation_set_is_the_one_the_story_selected`) reaches 10
  of 52 files. There is no template here to extract.
- **22.7% of the corpus is prose** — 1,598 lines of `//!` module documentation, 3,154 of `///` item
  documentation, 368 of inline comment. That prose is the argument each assertion rests on.
- **All 52 files cite at least one story id**, 90 distinct ids across the corpus, median 5 per file.
  These files are the repository's decision record, indexed to the decisions.
- **29 of 52 cite C-30** — the query-encoding gap — and 37 carry a `no_…` test asserting a
  deliberate *absence*. A generator emits what a provider declares; it cannot emit what a provider
  was deliberately not allowed to declare, which is what half this corpus is about.

Five claims of the Slack grade, each verbatim, each catching something nothing else would:

- **`slack_connector.rs`** — Slack declares no query parameter at all, because its ids are opaque and
  the emitter percent-encodes nothing (C-30): *"nothing else in the repository would fail if someone
  converted a read to a GET, and the resulting connector would look tidier and be broken."*
- **`sentry_connector.rs::the_emitted_url_of_every_operation_is_pinned_including_its_trailing_slash`**
  — *"Sentry's trailing slash is part of the address: a `GET` without it is a 301 that can lose the
  Authorization header and a `PUT` without it is a 404. Nothing else in this repository fails when it
  is dropped."* Its doc adds why `ends_with('/')` is not enough: that property also passes for the
  issue *list* endpoint and for a paginated event list — "both plausible outcomes of an edit that
  meant no harm".
- **`google_connector.rs::no_google_body_field_is_optional`** — *"Drive's `files.update` is worse
  than a rejection, because a field that *is* nullable is **cleared** by one. A connector that
  offered the field would therefore destroy data because the caller left it alone, which is the worst
  available failure mode."* Data loss caused by a caller doing nothing.
- **`hubspot_connector.rs::every_hubspot_write_wraps_its_fields_in_the_properties_envelope`** — *"a
  flat body is accepted, ignored and answered 2xx, so nothing but this assertion distinguishes a
  write that stores the caller's data from one that stores nothing."*
- **`zoom_connector.rs::the_meeting_settings_object_is_declared_through_wire_paths`** — *"a flattened
  one would bind `payload = { …, waiting_room: …, … }`, which parses, analyzes and is canonical, so
  nothing but this assertion would fail."* Zoom ignores undefined top-level members and answers 201.
- **`discord_connector.rs::the_emitter_never_reads_the_credential_declaration`** — the file that
  caught *its own* inert test: a text search for the auth prefix "could not have failed … before the
  change or after it. It was reassurance wearing an assertion's clothes." It now emits against a
  credential-stripped connector and demands byte-identical output.
- **`airtable_connector.rs::every_airtable_path_value_is_alphanumeric_by_declaration`** — the only
  argument in the repository about **path** safety rather than query safety: a table *name* is
  arbitrary user text, so `A/B tests` addresses a different route, and the `tbl` prefix is what
  refuses the name form.

### The two suspicions, checked

**Tautology.** **194 of 2,352 sites (8.2%)** cannot fail on a connector that loaded — smaller than
suspected, and not where it was expected. The largest classes are 73 restatements of
`program.ops[0].name == operation.id` (the emitted name *is* `operation.id` by construction), 27
`!field.label.is_empty()` / `!field.help.is_empty()` and 22 secret-field-has-no-example checks (the
loader refuses all three: `config_fields.rs`), and 25 `user_env.is_empty() && user_suffix.is_none()`
on a non-basic scheme (`validate_credentials` refuses that too).

A correction the epic's premise needs: **`connector-cli -- diff` does not subsume these.** `diff`
and `shipped_modules.rs::every_shipped_operation_is_byte_identical_to_its_committed_rendering` pin
the emitted *text* against the committed rendering. They say nothing about whether that text has any
particular property, so they would accept a committed rendering carrying a `?` just as happily. The
Slack-family "no query string reaches the URL" assertions are **not** covered by `diff`, and the
tautology argument does not reach them.

**Fixture drift.** **Zero of 52** files assert about a fixture in place of the shipped provider.
C-421 closed this class before this measurement ran. The three files that do not route a load
through `shipped_provider::*`:

- `algolia_connector.rs` and `linear_connector.rs` use fixtures because **no `providers/algolia.toml`
  or `providers/linear.toml` exists.** They are recorded negative-result probes, and each asserts the
  absence outright — `no_provider_toml_was_shipped_for_this_probe` is algolia's last test. They are
  bucket (c) end to end (algolia 69%, linear 81% by assertion), not drift.
- `sendgrid_connector.rs` reads the real `providers/sendgrid.toml` off disk but through
  `provider::load` rather than the C-421 helper. What it asserts is asserted about the shipped bytes;
  the miss is that it will break when SendGrid converts to `[spec]`. That is a one-line C-421
  follow-up, not a measurement finding.

### The conclusion

**Do not generate per-connector tests.** Bucket (c) is the majority on every measure — 52% of
assertions, 50% of lines, 66% of lines inside test functions — and the mechanical remainder is
smaller than the epic assumed and cheaper to leave alone than to automate. The premise that 22,455
lines are boilerplate is false: the boilerplate is 656 lines of constants, 2.9%.

The one genuinely mechanical population is bucket (b): 209 assertions and 1,571 lines restating
`shipped_modules.rs`, concentrated in a near-identical `every_<x>_operation_emits_an_analyzable_module`
test carried by roughly 40 of the 52 files. Even that is under 7% of the corpus, and deleting it has
a stated cost — `slack_connector.rs` restates the fleet gate **on purpose**, "so that the Slack
connector's own test file fails on its own when the module stops being analyzable". C-424 is
therefore worth at most a small, careful pass, not the rewrite the epic imagined.

There is also a structural reason generation would fight the repository: C-230's
`crates/connector-cli/tests/per_provider_test_scope.rs` already governs what a per-connector file may
assert — it refuses a `providers/` walk inside one, because a catalogue-wide claim written in a
worktree holding one provider turns another implementor's merge red. The per-connector files are, by
an enforced rule, *about their provider*. That is the opposite of a generated population.

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
