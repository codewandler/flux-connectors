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

## It is not hypothetical — four shipped providers do it today

The first draft of this story assumed no shipped provider was the wrong shape. That was wrong, and
checking it is what turned a latent bug into a measured one. Six providers declare `[[services]]`;
five have more than one; and **every one of those five leaks**:

| provider | services | leaks | what crosses the boundary |
|---|---|---|---|
| `anthropic` | `models`, `admin` | config + verify | `admin_key` reaches `--service models`; `verify = "anthropic-models-list"` reaches `--service admin` |
| `contentful` | `delivery`, `management` | config + verify | `delivery_token` and `management_token` each reach the other; `verify` is a `delivery` operation |
| `postmark` | 2 | config | per-service config crosses |
| `microsoft_graph` | 3 | verify | `verify = "microsoft_graph-calendar-calendar-get"` reaches `mail` and the third service |
| `google` | 3 | — | declares neither, so nothing to leak |

Two of those values are **secrets** — `admin_key` and `contentful`'s per-service tokens — which is
what moves this from untidy to worth doing before C-87 rather than after. Measured by reverting the
fix and running the new shipped-provider test:

```
`anthropic --service models` carries configuration field `admin_key`, which configures service `admin`
`anthropic --service admin` keeps `verify = "anthropic-models-list"`, an operation it no longer declares
```

Nothing declares `[[graphs]]` anywhere in `providers/`, so that third surface has no shipped case and
is asserted against a fixture.

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
`connectors.lock` — whose hash domain *does* include all three (`ir.rs:1290-1317`) — is still
unwired (C-7). So `flux-connectors diff` is a fixed point before and after this fix, and that is the
regression proof rather than a caveat.

[C-87](C-87-configuration-codegen.md) is `ready` and its whole subject is publishing the
configuration surface. The day it lands, `flux-connectors build --service <s>` writes another
service's configuration fields — labels, help text, `binds` destinations, secrecy flags — into a
shipped artifact, and the resulting diff will point at C-87, which will not have caused it. Fixing it
here costs one filter each and two tests; fixing it after C-87 costs an investigation first.

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
- [x] **No artifact moves.** `cargo run -p connector-cli -- diff` reports a fixed point, and the
      committed tree is unchanged. This fix alters no emitted output today, which is the whole
      argument for landing it early. → `454 artifacts up to date (41 providers checked)`, and
      `git status --short` names only `seam.rs` and this story.

## Notes

- The bug is invisible to every shipped provider, which is why no existing test caught it: no
  multi-service provider in `providers/` declares `[[config]]` or `[[graphs]]` today
  (`google.toml` and `microsoft_graph.toml` are the three-service cases and neither does). The test
  therefore needs a constructed fixture, loaded through the real `connector_spec::provider::load` so
  that the *starting* value is a connector the loader accepts.
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
- **Why the fixture is constructed rather than a shipped provider.** `providers/google.toml` and
  `providers/microsoft_graph.toml` are the three-service cases and neither declares `[[config]]` or
  `[[graphs]]` — and **no shipped provider declares `[[graphs]]` at all**: the single occurrence of
  the word anywhere in `providers/` is a comment at `stripe.toml:44` listing the keys a provider file
  may carry. So there is no shipped shape in which this bug is visible, which is also the reason it
  survived C-83, C-87's design and two service-narrowing test suites.

**Base proof.** At `f282e0a` (`git merge-base main HEAD`), with the test present and the fix absent:

```
$ cargo test -p connector-cli --lib seam::tests::selecting_a_service
test seam::tests::selecting_a_service_carries_no_other_services_config_graphs_or_verify ... FAILED
assertion `left == right` failed: `--service s3` carries another service's configuration fields
  left: ["bucket", "region"]
 right: ["bucket"]
```

It fails on `config` first, as the acceptance requires. `selecting_a_service_keeps_the_connector_level_surfaces`
passes at the base — correctly, since it asserts what the tail already did right.

**No artifact moved**, which was the falsifiable half of the "harmless today" claim rather than an
assumption: `cargo run -p connector-cli -- diff` reports `454 artifacts up to date (41 providers
checked)` and `git status --short` lists only `crates/connector-cli/src/seam.rs` and this story.

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
