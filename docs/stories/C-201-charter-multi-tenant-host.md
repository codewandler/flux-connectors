---
id: C-201
title: "Amend the charter: a deployed multi-tenant host is in scope"
pillar: Bridge
status: in-progress
priority: 1
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, docs]
note: "owner-directed 2026-07-31. vision.md's non-goal and connectors-app.md's loopback narrowing both forbid what C-200 builds; without this the repository's own contract reads as violated"
---

# Amend the charter: a deployed multi-tenant host is in scope

## Goal

Make [C-200](C-200-connectors-api-epic.md) legal in the repository's own terms, and redo the analysis
that the narrowing was standing in for — so the service ships with its risks named rather than with
its charter quietly out of date.

## Why this is a story and not a paragraph in a PR

Three documents currently forbid this service, each for a stated reason:

- `docs/vision.md` — *"A runtime for production traffic… Loopback-bound, never published, never a
  production request path."*
- `docs/designs/connectors-app.md` — the narrowing table: callers are *"the operator sitting in front
  of it"*, credential scope is *"one operator's own, in one process they started"*, binding is
  *"loopback only, no configuration to change it"*, and *"the first PR that adds a `--bind` flag is
  the one to refuse."*
- `docs/designs/connectors-proxy.md` — *"a credential-injecting proxy is, by construction, a
  confused-deputy machine: its entire job is to add authority a caller does not have."*

The third is not a preference. It is the argument C-34 resolved, and a multi-tenant service reopens
it in full: the service holds many tenants' credentials and adds authority to a caller who does not
hold them. Amending the first two without answering the third would delete the analysis rather than
address it.

## Acceptance

- [x] `docs/vision.md`'s non-goal is amended to permit a deployed multi-tenant host, naming what is
      still out of scope. The amendment states what changed and who directed it.
- [x] `docs/designs/connectors-app.md` is marked **superseded by** `connectors-api.md`, not deleted.
      Its slice-1 sequence and its `Egress`/transport analysis remain correct and are cited by C-202
      and C-203.
- [x] `docs/designs/connectors-api.md` carries a **Confused deputy** section that answers
      `connectors-proxy.md` directly: who the principal is, what proves a caller may use a tenant's
      credential, what stops tenant A's session reaching tenant B's secret, and what the service
      refuses to do. "It is authenticated" is not an answer on its own — the proxy design already
      rejected that as the mitigation.
- [x] `AGENTS.md`'s **Intentional gaps** entry for "nothing here makes a live call, because nothing
      here is a host" is updated rather than left standing, since it stops being true.
- [ ] No document in the repository still asserts the service may not exist. Checked by grep for
      `loopback`, `never published`, and `production request path`.
      **Not met — one file left, and it is fenced.** `docs/roadmap.md:29-32` still reads *"this
      workspace links no `http.request` implementation … and runs no host process. That is the
      loopback-only reference host the vision's narrowed non-goal now permits — `crates/connectors-app`"*.
      Both halves are now false. The roadmap is the coordinator's to write; see §Progress for the
      replacement text.

## Notes

- Keep `publish = false` on the service crate regardless. The amendment is about *deployment*, not
  about putting a host on crates.io; the publish closure stays four crates
  ([C-190](C-190-publish-catalog-pack-secrets.md)).
- The narrowing that should survive verbatim: **the service constructs no request of its own.** That
  was the structural reason `connectors-app` superseded `connectors-proxy`, and it is unaffected by
  tenancy.

## Progress

**2026-07-31 — amendment landed; one fenced file outstanding.**

The design named in this story's frontmatter, `docs/designs/connectors-api.md`, **did not exist**. It
was created here rather than stalling, scoped to the charter half only — what the amendment permits,
the bind gate, and the deputy answer. The engineering design (tenancy model, routes, sign-in) is left
to C-202–C-204 so this does not collide with the epic's own stories.

**The finding that shaped the amendment.** The dispatch described the shipped crate as "a deployed
multi-tenant host". It is not one yet, and writing the charter as though it were would have replaced
one false document with another. Measured at `1390f09`:

- `src/main.rs` binds `Ipv4Addr::LOCALHOST:8787` with **no flag and no env var** — the loopback
  narrowing is still literally true of the code.
- `src/api.rs:24` is `const SOLE_TENANT: &str = "local"`, and `tenant_of()` returns it. **There is no
  authenticated principal**, so the deputy mitigation the charter needs does not exist in code yet.
- `publish = false` holds; the credential store is in-memory.

What is genuinely contradicted today is narrower than "multi-tenancy": the repo now **has a host**,
**links a transport**, **opens sockets**, and **has made a real vendor call** — against docs saying
none of that existed. Multi-tenancy is contradicted only in *shape* (the tenant is a parameter of
every port rather than a global).

So the amendment permits the deployed multi-tenant destination, records the three rows that are
already the deployed shape and the three that are not, and converts the old "no `--bind` flag, ever"
prohibition into a **four-item gate** on when the bind may widen — item 1 being that `tenant_of()`
must read a verified session first. A PR adding `--bind` while the tenant is a constant is still the
rejected proxy, and the gate says so in those words.

The deputy answer does not lean on "it is authenticated". It turns on the **interface**: the caller
names an operation id and cannot name a host, a credential, or a tenant. A deputy adds authority its
caller lacks; this returns authority its caller deposited.

**Remaining, for the coordinator** (fenced from this story): `docs/roadmap.md:29-32`. Suggested
replacement — *"A live call is no longer gated: `codewandler-flux-web` supplies the `http.request`
implementation and `crates/connectors-api` is the host that binds it. See
[designs/connectors-api.md](designs/connectors-api.md); the loopback narrowing in
[designs/connectors-app.md](designs/connectors-app.md) is superseded by C-201."*

Also outstanding and deliberately not touched: `crates/connectors-api/README.md:97-103` says the
crate "contradicts `docs/vision.md`'s current non-goal" and that "this README is the only place
saying so". Both stop being true when this lands.

## Coordinator note at integration (2026-07-31)

Returned `PARTIAL` on one item and the item was correct: Acceptance 5 ("no document still asserts the
service may not exist") was blocked by `docs/roadmap.md`, which is a fenced ledger. **Applied at
integration, so the item is now met** — the roadmap's "links no `http.request` implementation … runs
no host process" paragraph is replaced, and it now also carries the charter-vs-code gap rather than
letting a reader infer a deployment that does not exist. Every remaining grep hit for the old
narrowing is the amendment *quoting* the text it supersedes.

**The implementor corrected the dispatch, and was right to.** I described the shipped crate as "a
deployed multi-tenant host"; it is not one. `src/main.rs` binds `Ipv4Addr::LOCALHOST:8787` with no
flag and no env var, and `src/api.rs:24` is `const SOLE_TENANT: &str = "local"`. Writing the charter
as though the deployment had been reached would have replaced one false document with another. The
amendment instead permits the destination and records which three rows of the six are already the
deployed shape.

Two write-set deviations accepted rather than bounced: `docs/designs/connectors-api.md` (Acceptance
item 3 names it, five stories' `design:` frontmatter pointed at it, and it did not exist) and
`AGENTS.md` (Acceptance item 4 names it explicitly). Both were required by the story's own
Acceptance, so the narrower write set in my dispatch was the error.

**C-200's child table reconciled here**, since it is a fenced story file: it named C-205–C-208 for
its own children, but C-205 and C-206 were already filed as unrelated stories and C-207 is now the
credential-persistence story. The four unfiled children are renumbered C-208–C-211 and written as
plain text rather than links, because a link to a story that does not exist is exactly how the table
came to name four that never resolved.

**Carried forward, not fixed:** `crates/connectors-api/README.md:97-103` still says the crate
"contradicts `docs/vision.md`'s current non-goal" and that "this README is the only place saying so".
Both stopped being true when this merged. It is left alone deliberately — `crates/connectors-api/**`
is owned exclusively by the running C-204 implementor this wave, and editing it here would race that
agent. It will be corrected at C-204's integration.

**Risk recorded rather than closed:** the four-item bind gate is prose enforced by review only. The
mechanical control — a test that fails when the bind is non-loopback while `tenant_of()` returns a
constant — does not exist and is not filed. It is named in the design's Risks section and is the
first thing to look at if a deployed host ever leaks a credential.
