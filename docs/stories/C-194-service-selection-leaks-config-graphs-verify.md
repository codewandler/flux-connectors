---
id: C-194
title: "`select_service` narrows four surfaces and carries `config`, `graphs` and `verify` through unfiltered"
pillar: Build
status: in-progress
priority: 2
epic: connectors-v1
areas: [connector-cli]
note: "found while mapping the connector surface model. The `..connector.clone()` tail at seam.rs:263 copies three service-partitioned surfaces whole, so the narrowed IR is one the loader would refuse. Harmless only because none of the three reaches an artifact yet — C-87 publishes exactly that surface, and the diff will read as a C-87 regression"
---

# `select_service` narrows four surfaces and carries `config`, `graphs` and `verify` through unfiltered

## Goal

Make a `--service` narrowing carry the selected service's surfaces and nothing else, so that the
configuration codegen C-87 lands on top of a narrowing that is already correct rather than inheriting
a leak and being blamed for it.

## What was measured

`select_service` (`crates/connector-cli/src/seam.rs:226-265`) builds the narrowed `Connector` by
filtering four fields and spreading the rest:

```rust
Ok(Connector {
    services: …filter(|declared| declared.name == service)…,
    operations: connector.operations_of(service).cloned().collect(),
    events:     connector.events_of(service).cloned().collect(),
    channels:   connector.channels_of(service).cloned().collect(),
    ..connector.clone()
})
```

`Connector` carries **seven** service-partitioned or service-derived surfaces, not four. The tail
carries three of them through whole:

| field | what makes it a service's own | accessor | today |
|---|---|---|---|
| `operations` | `Operation::service` | `Connector::operations_of` (`ir.rs:1000`) | filtered |
| `events` | `EventDecl::service` | `Connector::events_of` (`ir.rs:1008`) | filtered |
| `channels` | `ChannelBinding::service` | `Connector::channels_of` (`ir.rs:1015`) | filtered |
| `config` | `ConfigField::service` (`config.rs:287`) | **`Connector::config_of` (`ir.rs:1022`)** | **whole** |
| `graphs` | `Graph::service` (`graph.rs:438`) | **`Connector::graphs_of` (`ir.rs:1034`)** | **whole** |
| `verify` | names one operation, and an operation has exactly one service | *none exists* | **whole** |

Two of the three already have the accessor the fix needs, unused. The comment sitting directly above
the leak states the rule it does not follow: *"The three kinds partition the same way for the same
reason — each member names exactly one service — so one filter per kind is the whole rule."* C-83
wrote that when there were three kinds; `config` and `graphs` arrived later and were not added.

## It is not hypothetical — four shipped providers leak, 17 times

The first draft of this story assumed no shipped provider was the wrong shape. **That assumption was
wrong**, and checking it is what turned a latent bug into a measured one. Of 43 providers, six declare
`[[services]]` and five have more than one. Measured by reverting the fix and running the new
shipped-provider test, which collects every violation rather than stopping at the first:

```
anthropic --service models: configuration field `admin_key` configures service `admin` (declares secret = true)
anthropic --service admin: configuration field `api_key` configures service `models` (declares secret = true)
anthropic --service admin: `verify = "anthropic-models-list"` names an operation it no longer declares
contentful --service delivery: configuration field `management_space_id` configures service `management`
contentful --service delivery: configuration field `management_environment_id` configures service `management`
contentful --service delivery: configuration field `management_token` configures service `management` (declares secret = true)
contentful --service management: configuration field `delivery_space_id` configures service `delivery`
contentful --service management: configuration field `delivery_environment_id` configures service `delivery`
contentful --service management: configuration field `delivery_token` configures service `delivery` (declares secret = true)
contentful --service management: `verify = "contentful-entries-list"` names an operation it no longer declares
microsoft_graph --service mail: `verify = "microsoft_graph-calendar-calendar-get"` names an operation it no longer declares
microsoft_graph --service calendar: configuration field `access_token` configures service `mail` (declares secret = true)
microsoft_graph --service files: configuration field `access_token` configures service `mail` (declares secret = true)
microsoft_graph --service files: `verify = "microsoft_graph-calendar-calendar-get"` names an operation it no longer declares
postmark --service server: configuration field `account_token` configures service `account` (declares secret = true)
postmark --service account: configuration field `server_token` configures service `server` (declares secret = true)
postmark --service account: `verify = "postmark-deliverystats-get"` names an operation it no longer declares
```

**12 config crossings and 5 `verify` crossings, across `anthropic`, `contentful`, `microsoft_graph`
and `postmark`.** `google` is multi-service and declares neither, so it has nothing to leak, and
`statuspage` and `okta` are single-service. Nothing in `providers/` declares `[[graphs]]` at all, so
that third surface has no shipped case and is asserted against a fixture instead.

## What "8 declare `secret = true`" does and does not mean

Eight of the twelve config crossings involve a field with `secret = true`. **That is a leak of a
declaration, not of a credential value, and the distinction is the whole severity question.**

A `ConfigField` is *a question a settings page asks*: `name`, `label`, `help`, `format`, `example`,
`docs_url` and `binds`. It has no value field, and it cannot acquire one — AGENTS.md's rule is that
**no credential value enters provider TOML, generated Flux, a manifest, the public catalogue or the
lockfile**, and `providers/*.toml` is compiler input written by hand. `secret = true` is a *claim
about the value a host will later collect* — "mask this on input, keep it out of logs" — and the
loader forces it to agree with `binds` precisely so it stays a claim rather than a second source of
truth.

So the worst thing the leak could have published, had it reached an artifact, is a **form field**:
the string `admin_key`, the label "Admin API key", its help text and the credential name it binds to.
Credential *names* are already public by design — `web/public/catalog.json` publishes
`anthropic.admin_key` today under `/providers/1/auth/credentials/1/name`, and that is intended.

That is a real defect worth fixing — a `models`-scoped install asking an operator for an admin key is
wrong, and wrong in a way that erodes the operator/connection level split the configuration contract
exists to enforce. **It is not a credential disclosure**, and this story should not be read as one.

## Nothing is on disk today, and that is checkable rather than assumed

**No committed artifact contains any of it.** The three surfaces reach no emitter:

- The manifest serializes a fixed struct with no `config`, `graphs` or `verify` field
  (`seam.rs::manifest`), and `grep -c '^verify\|^config\|^graphs' connectors/anthropic-*.connector.toml`
  is `0`.
- `web/public/catalog.json` has **no `verify` and no `graphs` key anywhere**, and its only `config`
  key is a *vendor response schema property* at `/providers/13/operations/{2,4}/response_schema/properties/config`
  — unrelated to this surface.
- The decisive check: a distinctive config `help` string —
  `"Create an Admin API key in the Anthropic Console…"` — appears nowhere in `connectors/`,
  `web/public/` or `crates/catalog/`. It exists only in `providers/anthropic.toml`, which is input.
- `connectors.lock` would carry all three in its hash domain (`ir.rs::HashDomain`), but it is never
  written — that is C-189's whole subject.

There is also a structural reason the leak could not have reached the whole-catalogue artifacts even
if they did publish config: `catalog.json` and the provider index are written **only on a full run**,
and a full run never calls `select_service` at all. The leak is reachable only through
`--service`-scoped runs, which by design write per-service files only.

## The narrowed value is one the loader would refuse

This is the sharper statement, and it is what makes the defect structural rather than cosmetic. Two
recorded refusals hold on a loaded connector and **stop holding** on the value `select_service`
returns:

1. **"A connector asks for nothing it cannot use."** `validate_config` refuses a field binding
   `endpoint.{variable}` that no service `base_url` carries (`provider.rs:557-573`). Narrow a
   two-service provider to the service whose base URL is literal, and the *other* service's
   endpoint-bound field survives — binding a variable the narrowed connector no longer has anywhere.
2. **`verify` must name a declared operation.** `validate_verify` (`provider.rs:664-687`) refuses a
   `verify` that no `[[operations]]` block declares. Narrow away the service that owns the verify
   operation and the narrowed connector names an operation it does not declare — a "Test connection"
   pointer into a service this build is not producing.

So every backend downstream of the narrowing is handed an IR that violates the loader's own
invariants. The narrowing exists precisely so that *"every backend then sees a connector that simply
has one service"* (`seam.rs:218-221`), and for three surfaces that sentence is false.

## Why now rather than as part of C-87

**It emits nothing today.** `config`, `graphs` and `verify` reach no artifact: the manifest carries
operations, events and channels only (`seam.rs:354-426`), `catalog.rs`, `site.rs` and
`connector-flux/src/op.rs` name the three fields only inside their own test fixtures, and
`connectors.lock` — whose hash domain *does* include all three (`ir.rs:1290-1317`) — is never written,
which is [C-189](C-189-the-lockfile-is-never-written.md)'s subject. So `flux-connectors diff` is a
fixed point before and after this fix, and that is the regression proof rather than a caveat.

[C-87](C-87-configuration-codegen.md) is `ready` and its whole subject is publishing the
configuration surface. The day it lands, `flux-connectors build --service <s>` writes another
service's configuration fields — labels, help text, `binds` destinations, secrecy flags — into a
shipped artifact, and the resulting diff will point at C-87, which will not have caused it. Fixing it
here costs one filter each and two tests; fixing it after C-87 costs an investigation first.

**Two other ready stories widen the blast radius the same way**, and both are in `main` now:
[C-189](C-189-the-lockfile-is-never-written.md) would start writing `connectors.lock`, whose hash
domain already includes all three surfaces — so a scoped run would record a hash computed over
another service's config; and [C-190](C-190-publish-catalog-pack-secrets.md) publishes secrets
metadata into the pack. Whichever of the three lands first is the one that turns this from latent into
emitted, which is the argument for it not being any of their problem.

## Acceptance

- [x] `config` and `graphs` are narrowed through the accessors that already exist —
      `Connector::config_of` and `Connector::graphs_of` — rather than through a second filter written
      at the seam. A duplicated predicate is how the partition rule comes to be stated in two places
      and disagree. → `crates/connector-cli/src/seam.rs:265-266`
- [x] `verify` becomes `None` when the operation it names does not belong to the selected service,
      and is preserved when it does. **Record the reasoning**: `verify` is connector-level but
      *denotes* an operation, and an operation has exactly one service, so it is service-derived. The
      alternatives are both worse — carrying it names an undeclared operation (`validate_verify`
      refuses that on load), and dropping it unconditionally would strip a legitimate "Test
      connection" from the very service that owns it. → `seam.rs:267-277`, with the reasoning as the
      comment directly above the filter
- [x] **Failing-first test:**
      `crates/connector-cli/src/seam.rs::selecting_a_service_carries_no_other_services_config_graphs_or_verify`.
      It narrows a two-service fixture — each service declaring its own `[[config]]` field, its own
      `[[graphs]]` flow, with `verify` naming an operation of one of them — in **both** directions,
      and asserts on all three surfaces. It must fail at the merge base, and on `config` first.
      → `seam.rs:973-1046`; at `f282e0a` it fails with
      ``left: ["bucket", "region"] / right: ["bucket"]``, which is `config`, first
- [x] The narrowed connector satisfies the invariants stated above, asserted as such and not only
      field-by-field: every surviving `config` field and `graph` names the selected service, and
      `verify` is either `None` or resolves through `selected.operation(...)`. → `seam.rs:1013-1027`
- [x] `auth`, `default_auth`, `base_url`, `description` and `vendor` still come through the tail
      unchanged, and a test says so. They are connector-level: `AuthMethod` carries no `service`, and
      a service resolves its own `base_url` through `base_url_of`. Widening the fix to them would be
      a different story with a reachability computation in it.
      → `seam.rs::selecting_a_service_keeps_the_connector_level_surfaces`
- [x] **No artifact moves *because of this diff*.** This fix alters no emitted output, which is the
      whole argument for landing it early. → Pre-merge, on a clean base, `diff` reported
      `454 artifacts up to date (41 providers checked)`. Post-merge it reports
      `2 artifacts would change (43 providers checked)` — `crates/catalog/src/generated.rs` and
      `web/public/catalog.json`, **both fenced and both stale at the merge base**: reverting this
      fix leaves the same two files stale, and `select_service` is unreachable from a full build
      (`pipeline.rs:205`), which is what `diff` checks. `git status --short` names only `seam.rs`,
      `tests/service_units.rs` and this story.

## Notes

- **The reason no existing test caught it is worth recording, because it is not "no provider is that
  shape".** Four providers *are* that shape and leak 17 times. Nothing caught it because no test ever
  looked at `select_service`'s output beyond `operations` — `selecting_a_service_drops_every_other_operation`
  checks the operation ids and the emitted module, and the emitted module cannot show a config leak
  because the emitters do not read config. **The surfaces that reach no artifact are exactly the
  surfaces no artifact test can cover**, which is a gap that will recur for any future IR-only field.
- The fixture is still constructed as well as measured against shipped providers, because
  `[[graphs]]` has no shipped case at all and only a fixture can exercise it. It is loaded through the
  real `connector_spec::provider::load`, so the *starting* value is a connector the loader accepts.
- Do not fix this by teaching each backend to filter. The design note at `seam.rs:217-221` is
  explicit that narrowing the IR once is what keeps "the other service's members are absent" true for
  the module, the manifest, the catalog and the site document at once, "with no per-backend filter to
  forget". A per-backend filter is the failure mode, not the fix.
- `..connector.clone()` is the mechanism, and it is worth noting it will do this again: a new
  service-partitioned field is carried through silently, because the spread compiles. `HashDomain::of`
  (`ir.rs:1319-1345`) solves the same problem for the hash domain with an exhaustive destructuring
  that fails to compile until someone states the answer. Whether `select_service` should be written
  the same way is worth a decision; this story does not take it, because the tripwire and the fix are
  separable and the fix is the urgent half.

## Progress

Done in `crates/connector-cli/src/seam.rs` (branch `impl/seam-service-leak`). One file changed; no
artifact moved.

**The fix is three lines of narrowing plus the fixture that makes them observable.** `config` and
`graphs` go through `config_of`/`graphs_of`, which existed and were unused. `verify` is
`connector.verify.clone().filter(|id| …operation(id).is_some_and(|op| op.service == service))` — it
survives exactly when its operation does, which is the only reading that leaves the narrowed value
loadable in both directions.

**Everything the acceptance asked to be recorded:**

- **Why `verify` is service-derived rather than connector-level.** It holds an operation id, and
  `Operation::service` is a single concrete name, so `verify` inherits that service transitively.
  Neither "always keep" nor "always drop" is defensible: keeping it across a boundary produces a
  connector naming an operation it does not declare — `validate_verify` (`provider.rs:664-687`)
  refuses exactly that on load — and dropping it always would remove a legitimate Test-connection
  button from the service that owns the operation. Both directions are asserted.
- **Why `auth` and `default_auth` were left in the tail.** `AuthMethod` has no `service` field at all
  (checked: `grep service crates/connector-spec/src/auth.rs` is empty) and an `AuthRequirement` names
  a credential connector-wide, so there is no partition to filter on — narrowing them would mean
  computing which credentials the surviving operations can reach. That is a different story, and
  `selecting_a_service_keeps_the_connector_level_surfaces` is the test that stops a later edit from
  taking it by accident.
- **Why there is a fixture *and* a shipped-provider test.** The shipped test is the one that matters
  — it found the 17 — but it cannot cover `graphs`, because **no provider declares `[[graphs]]` at
  all**: the single occurrence of the word anywhere in `providers/` is a comment at `stripe.toml:44`
  listing the keys a provider file may carry. The fixture covers that third surface and pins both
  narrowing directions of `verify` deterministically.
- **Correcting this story's own first draft.** It asserted that no shipped provider was the wrong
  shape and that the bug was therefore invisible. The first claim was false and I did not verify it
  before writing it down; the shipped-provider test exists because checking it was the obvious next
  step and it immediately produced 17 violations. The *conclusion* — nothing reaches an artifact —
  survived the correction, but it now rests on four direct checks rather than on the assumption.

**Base proof.** At `21ddf05` (`git merge-base main HEAD`, after merging main), with both tests present
and the fix absent:

```
$ cargo test -p connector-cli --lib seam::tests::selecting_a_service
test seam::tests::selecting_a_service_carries_no_other_services_config_graphs_or_verify ... FAILED
assertion `left == right` failed: `--service s3` carries another service's configuration fields
  left: ["bucket", "region"]
 right: ["bucket"]

$ cargo test -p connector-cli --test service_units narrowing_a_shipped_provider
test narrowing_a_shipped_provider_carries_no_other_services_config_graphs_or_verify ... FAILED
a service-scoped narrowing carried 17 surface(s) belonging to another service:
  [the 17 enumerated above]
```

The unit test fails on `config` first, as the acceptance requires.
`selecting_a_service_keeps_the_connector_level_surfaces` passes at the base — correctly, since it
asserts what the tail already did right, and a failing-first test that also fails for the untouched
half would not be isolating anything.

**No artifact moved.** This was the falsifiable half of the "harmless today" claim, not an assumption:
`cargo run -p connector-cli -- diff` reports a fixed point over the full merged catalogue, and
`git status --short` names only `crates/connector-cli/src/seam.rs`,
`crates/connector-cli/tests/service_units.rs` and this story. Combined with the four direct checks
above — no `verify`/`graphs` key in `catalog.json`, no such keys in any manifest, no config `help`
string anywhere outside `providers/`, and no lockfile — the "IR-only" claim is measured rather than
argued.

**The gate is red at the merge base, in the two coordinator-owned files, and I did not touch them.**
After `git merge --no-ff main` (merge base `21ddf05`), `cargo test --workspace --no-fail-fast` leaves
exactly the eight whole-catalogue staleness tests red that `AGENTS.md` tabulates —
`the_provider_list_matches_the_repository`, `the_catalog_is_not_empty`,
`the_committed_tree_is_a_fixed_point_of_a_build`, `a_build_plans_both_readme_images_and_they_are_current`,
`the_shipped_artifacts_are_byte_identical`, `the_published_catalogue_carries_the_service`,
`every_shipped_operation_carries_its_metadata_and_its_flux`, `the_build_writes_and_checks_site_catalog_json`.

`diff` names the cause: **2 artifacts would change (43 providers checked)** —
`crates/catalog/src/generated.rs` and `web/public/catalog.json`, both fenced. Traced to the base
rather than reasoned about: with this story's fix reverted in place, `diff` reports **the same two
files**, so the staleness is inherited. The structural reason it cannot be this diff is independent
and stronger — `select_service` is called only under `if let Some(selector) = service`
(`pipeline.rs:205`), so a full build never invokes it at all, and every artifact `diff` checks comes
from a full build.

Left for the coordinator to regenerate at integration, per the whole-catalogue rule. **Do not read
these eight as this story's regression.**

Two things a resuming agent — most likely C-87's — should know:

1. **`..connector.clone()` will do this again.** It is the mechanism, not the accident: a new
   service-partitioned field is carried through silently because the spread still compiles. The
   repository already solved this exact problem once, in `HashDomain::of` (`ir.rs:1319-1345`), with an
   exhaustive destructuring whose comment says so — *"a field added to `Connector` fails to compile
   here until someone states whether it belongs"*. Rewriting `select_service` the same way is the real
   fix for the class; this story deliberately did not, because the tripwire and the leak are separable
   and only the leak was urgent. It is worth filing.
2. **The `verify` narrowing has no accessor to hide behind.** `config_of` and `graphs_of` exist;
   nothing answers "does this service own the verify operation". If C-87 or anything else needs that
   question elsewhere, it should become `Connector::verify_of(service)` in `connector-spec` rather than
   a second copy of the predicate at another seam — the same argument that makes the first acceptance
   item insist on the existing accessors.
