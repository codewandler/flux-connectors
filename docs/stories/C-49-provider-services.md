---
id: C-49
title: Model a provider's services as the middle addressing level
pillar: Spec
status: in-progress
priority: 4
design: docs/designs/provider-services.md
epic: connectors-v1
areas: [connector-spec, connector-flux, connector-cli]
note: provider → service → operations · one service per operation · unset means `default`
---

# Model a provider's services as the middle addressing level

## Goal
Give a provider an explicit **service** level between the connector and its operations — `s3` and
`bedrock-runtime` under AWS, `support` under Zendesk — so that a service is the unit you address,
version, select and install, and every operation belongs to exactly one of them. This is the "scope"
or "group" C-37 sketched as a bare path segment, promoted to a named thing with an owner.

## Acceptance
- [x] **`Service` is an IR level, not a tag on the operation.** `Connector` gains
      `services: Vec<Service>`; `Service` carries `name`, `description`, an optional `base_url`
      override and an optional `api_version`. `Operation` gains `service: String`. A free-form
      `tags` field is explicitly rejected in the design: a tag cannot partition, version or host.
      → `crates/connector-spec/src/ir.rs` (`Service`, `Connector::services`, `Operation::service`);
      the rejection is `docs/designs/provider-services.md` §"`Service` is an IR level".
- [x] **Exactly one service per operation, and services partition the operation set.** A property
      test asserts the per-service operation sets are pairwise disjoint and their union is every
      operation — the invariant that makes "install the whole s3 service" a well-defined set.
      → `crates/connector-spec/tests/service_partition.rs::services_partition_the_operation_set`,
      over 200 generated shapes; the accessors are `Connector::service_names`/`operations_of`.
- [x] **`service` unset means `"default"`.** The name is reserved: no `[[services]]` entry may
      declare it, and an operation naming a service that no `[[services]]` entry declares is a loud
      error listing the services that do exist — following C-3's treatment of duplicate op ids.
      → `crates/connector-spec/src/provider.rs::validate_services` and
      `validate_operation_service`, with golden snapshots `tests/golden/undeclared-service.error` and
      `reserved-default-service.error`. Omitting `service` in a provider that declares named services
      is refused too, rather than falling into an undeclared `default`.
- [x] **Byte-identical output for today's three providers**, all of which are single-service and
      therefore all-`default`. Failing-first: a test pinning the four goldens in
      `crates/connector-flux/tests/golden/` and the generated `.flux`/`.connector.toml` artifacts as
      unchanged by this story.
      → `crates/connector-cli/tests/service_units.rs::the_shipped_artifacts_are_byte_identical`
      asserts every committed artifact except `catalog.json` is `Unchanged` by a rebuild, which covers
      all six providers' `.flux` and `.connector.toml`, every per-operation rendering and every
      generated catalogue table. `cargo run -p connector-cli -- build` rewrote exactly one file:
      `web/public/catalog.json`, which gains the service fields on purpose. The four goldens are
      byte-unchanged in the diff and remain pinned by `connector-flux`'s `op_emitter.rs`, whose only
      edit was naming the new field in its fixtures.
- [~] **The service is the first path segment of C-37's gid, and `default` is elided from it.**
      `com.amazonaws/s3:2006-03-01#object-get` · `com.zendesk.api/support/tickets:v2#show` ·
      `com.freshdesk.api/tickets:v2#create` (default elided, so C-37's variable depth still holds and
      `default` never reaches a published address). `parse(render(x)) == x` round-trips including the
      elision.
      → `crates/connector-spec/src/address.rs` (`Pid`/`Gid`/`Oip`) and
      `tests/service_partition.rs::addresses_round_trip_through_the_default_elision` (500 generated
      addresses), plus `the_validators_decide_which_addresses_round_trip` and
      `a_provider_file_that_loads_publishes_only_round_tripping_addresses`, which state the property
      over a corpus that includes the hostile spellings — the validators are the gate, and the loader
      enforces them (`address::validate_authority`/`validate_service_name`/`validate_api_version`).
      **`[~]` because one of the three published examples cannot be produced:**
      `com.zendesk.api/support/tickets:v2#show` has a `tickets` tail that only C-37's remaining path
      segments make, and the grammar implemented here *refuses* a gid with more than one middle segment
      rather than guessing — a tail plus the elision is genuinely ambiguous. Both admissible
      resolutions are recorded in the design and in the amendment note, and C-37 must choose one.
- [x] **`api_version` belongs to the service**, with the connector-level value as its default. AWS
      versions each service on its own date (`s3:2006-03-01`, `bedrock-runtime:2023-09-30`), so a
      single connector-level version cannot describe a multi-service provider.
      → `Connector::api_version_of` resolves service-then-connector;
      `tests/service_partition.rs::a_service_overrides_the_connector_version_and_base_url`.
- [~] **The emitted unit is the service.** A provider with named services emits
      `<provider>-<service>.flux` plus `<provider>-<service>.connector.toml` per service; a
      `default`-only provider still emits `<provider>.flux` exactly as today. `http_hosts` in each
      manifest derives from that service's own `base_url` and is never widened to `*` (C-10).
      → the emission half is done: `Workspace::service_module_path`, `seam::emit` returning one
      `ServiceArtifacts` per service, and
      `crates/connector-cli/tests/service_units.rs::a_two_service_provider_emits_one_pair_per_service`.
      **`http_hosts` itself could not be written — the manifest has no such field yet**, exactly as
      C-51 records: `<provider>.connector.toml` is C-10's placeholder. What exists today is the
      per-service `base_url`, which is the value C-10's allowlist derives from, and the test asserts a
      service's manifest carries its own base URL and never the other service's host.
- [x] **Building can select one whole service** — every operation belonging to it and nothing else —
      by service name or gid; an unknown service is a loud error naming the available ones. A test
      selects one service from a two-service fixture and asserts the other's operations are absent.
      → `--service <NAME|GID>` on `build`/`diff`, implemented as `seam::select_service` narrowing the
      IR (so every backend sees a one-service connector);
      `service_units.rs::selecting_a_service_builds_that_service_and_no_other` and
      `an_unknown_service_is_an_error_that_names_the_available_ones`.
- [x] **Service fields land inside `HashDomain::of`** — they are part of a connector's compiled
      meaning, like C-37's addresses and unlike C-7's provenance. C-2's determinism tests stay green
      unchanged.
      → `crates/connector-spec/src/ir.rs::HashDomain`, exercised by
      `tests/services.rs::every_service_field_is_inside_the_hash_domain`. The three new fields carry
      `skip_serializing_if` **inside** the hash domain, which is otherwise avoided there, so a
      connector declaring none of them hashes exactly what it hashed before this story — the
      alternative is every `connectors.lock` entry churning for a provider nobody edited.
      `determinism.rs` and `lockfile.rs` keep every assertion they had; the only edit is naming the new
      fields in their struct literals.
- [x] `docs/designs/provider-services.md` records the decisions; `docs/designs/global-addressing.md`
      gets an amendment note pointing at it, since this story fixes the meaning of its middle level.
      `AGENTS.md` records the partition invariant beside the auth conventions.
      → the design is written; `global-addressing.md` carries an amendment block at the top and is
      marked "amended by C-49"; `AGENTS.md` gains a **Service contract** section after the
      authentication contract. `docs/designs/catalog-json.md` documents the new published fields.

## Progress
- Filed from a user request on 2026-07-30 that named AWS (`aws` = provider, `s3`/`bedrock` = services)
  as the motivating case.
- **Implemented.** `Service` is an IR level; services partition the operation set; `default` is
  reserved, implicit and elided from both addresses and file names; `api_version` and `base_url`
  resolve service-then-connector; the emitted unit is the service; `--service <NAME|GID>` selects one
  whole service; the fields hash. All six shipped providers are single-service and their 59 committed
  artifacts are byte-identical — `flux-connectors diff` still reports
  `59 artifacts up to date (6 providers checked)`. The one regenerated file is
  `web/public/catalog.json`, which now carries a `services` array per provider and a `service` on every
  operation, additively (`schema_version` stays 2).
- **Two things a follow-up must pick up.** `http_hosts` could not be written because the manifest has
  no such field yet — see the `[~]` item; C-10 owns it and derives it from the per-service `base_url`
  this story added. And C-37 must resolve the tail-plus-elision ambiguity recorded in
  `docs/designs/provider-services.md` §Risks before adding path segments below the service; today a gid
  with more than one middle segment is refused.
- **Review round 1 found the validation gap and it is closed.** `[[services]].name`, `authority` and
  `api_version` were accepted unvalidated even though the grammar for them already lived in
  `address.rs`. Two consequences, both now fixed at the loader with the validators that module exposes:
  a service name reached the emitted file path (`name = "../../../../outside/pwned"` wrote *outside*
  the repository root, because a name flows into `artifact_stem` and `write_atomic` calls
  `create_dir_all`), and an unvalidated `authority = "com.acme/s3"` rendered `com.acme/s3:v2`, which
  reparses as a **different** address — falsifying the round-trip item while looking valid.
  **The invariant now restored and worth naming: no content field of a provider TOML influences an
  output path.** Before services, every path came from the discovered file stem. Pinned by
  `service_units.rs::a_service_name_cannot_write_outside_the_repository_root` and the golden
  `tests/golden/service-name-escapes-the-repo.error`. The property generators were the reason this was
  invisible — they drew only from hand-picked valid components — so they now draw from a mixed corpus
  with the validator as the gate.
- **Also from review:** a `--service` run no longer plans the provider-unit catalogue at all. Planned
  from a narrowed connector, `crates/catalog/src/generated/<provider>.rs` was *truncated* — the other
  service's rows dropped while their renderings stayed on disk, a stale catalogue that still compiles.
  It is now left alone exactly as `catalog.json` is, and
  `a_service_scoped_run_leaves_the_provider_unit_catalogue_alone` pins it.
- **For C-50 (AWS), the first multi-service provider:** `services.rs::every_shipped_provider_is_single_service`
  deliberately pins that no shipped provider declares services, so **C-50 must delete that test.** It
  exists to prove this story is meaning-preserving for today's catalogue, not to forbid tomorrow's.
- **Not done, deliberately:** the Rust catalogue (`crates/catalog`) still keys by provider rather than
  service. Splitting `ops/<provider>/` per service is a second reshape with no acceptance behind it, and
  the service travels in `catalog.json` where C-42's consumers can group by it. C-44's explorer UI is
  untouched; only the data it reads gained the field.

## Notes
- **Sequence this before C-37, or land the two together.** C-37 is `ready` at priority 5 and adds
  `Operation.path: Vec<String>` as anonymous hierarchy. If it lands first, this story reshapes those
  fields immediately and the address scheme is published twice — and C-37's own stability contract
  ("an oip, once published, is never reused") makes the second reshape expensive. That is why this
  sits at priority 4.
- **Why one service per operation rather than a set.** A set makes the gid ambiguous (which segment
  renders?) and makes selection non-partitioning, so "add s3" could no longer be answered by set
  membership. If an operation genuinely serves two services, it is duplicated deliberately with two
  ids — which is visible — rather than resolved by a rule nobody can see.
- **This is the slicing unit a 163-operation provider needs.** C-18 curated babelforce down to a
  handful precisely because a provider is not a usable tool catalogue; services make the cut
  structural instead of editorial, and give C-41's bundle layout its per-service directory.
- Write back to C-42's `catalog.json` schema and C-44's explorer: both currently group by provider,
  and service is the grouping a consumer wants. The schema must carry the service even if the UI
  follows later.
- The first multi-service provider, and the AWS-specific gaps it surfaces, are
  [C-50](C-50-aws-services.md).
