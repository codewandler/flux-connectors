# Design: connector contracts — substitutable capability contracts

**Status:** proposed — **hard-blocked on [C-23](../stories/C-23-operation-naming-contract.md), which
is `backlog` and never started; read §The prerequisite first** · **Pillar:** Spec (+ Bridge) ·
**Extends:** [provider-roles.md](provider-roles.md)

> Measurements below were taken on **2026-07-31** against the shipped catalogue —
> **41 providers, 232 operations, 48 services** — by reimplementing `fills_slot`
> (`crates/connector-spec/src/ir.rs:556`) over `web/public/catalog.json` and cross-checking with an
> independent `awk` pass. Citations into `../flux` were read at `codewandler-flux-lang` **0.39.0**.
> Re-grep by symbol; line numbers move.
>
> **The slot-fill figures below were not re-measured after C-161 (Okta) and C-181 (Statuspage)
> landed**, which took the catalogue to 43 providers / 242 operations / 50 services. Both are
> ordinary REST surfaces with `get`/`list` members, so every ratio here moves in the direction the
> argument already points — the slots get *looser*, not tighter. Re-run the measurement before
> quoting a number as current.

## Why

[provider-roles.md](provider-roles.md) is the prior art and most of its mechanism has landed:
[C-120](../stories/C-120-service-roles-declaration.md) is `done`, `Role` is a closed enum on a
service (`crates/connector-spec/src/ir.rs:501`), `Connector::missing_role_members` (`:983`) refuses a
claim the operations do not satisfy, and a provider's roles are derived rather than authored (`:951`).

But read what that design is actually *for*. Its own framing is discovery: *"a flow cannot ask 'who
can do this'"*, *"a UI cannot group them"*, *"a consumer reading the catalogue can rely on it without
reading the provider's TOML."* Every consumer it names **reads** a role. None of them **binds** one.

The owner's request is the next thing:

> *1password + vault — both secret stores — both expose a contract so either of them could be used
> when something requires the "secret store" contract.*

That is not discovery. It is **substitution**: a flow or an application declares that it requires a
secret store, an operator binds a concrete implementation, and the calls go through the *contract's*
member names rather than the vendor's. Discovery tells you 1Password is a candidate. Substitution is
what lets you swap it for Vault without editing the flow.

This document designs that, states what it needs first, and is honest that the prerequisite is a
story nobody has started.

## Two clarifications from the owner, both load-bearing

### Contracts span both repositories, and the registry lives in flux

A generated connector and a hand-written flux plugin can satisfy the **same** contract.
flux-connectors only **declares conformance**; the registry and the resolution live in flux.

That split is not a compromise, it is the only version that works — and it is worth stating in the
strongest terms because of what it implies about the charter.

`AGENTS.md:49-50` and `docs/vision.md:69-71` both put Vault firmly on the flux-plugin side:

> **Hand-written technology adapters belong in `../flux/plugins`:** Docker, Kubernetes, SQL,
> Prometheus, Loki, **Vault**, Asterisk, and other stateful or protocol-rich systems.

**That stays true, and nothing here changes it.** Vault satisfies the `secret_store` contract **as a
plugin**. No `providers/vault.toml` is proposed, implied, or made easier by this design. If a reader
comes away thinking Vault-the-connector is now in scope, this document has failed — the whole point
of putting the registry in flux is that a contract can be satisfied by an artifact this repository
does not and should not produce.

The division of labour follows:

| | flux-connectors | flux |
|---|---|---|
| declares conformance | **yes** — a service says which contracts it implements | yes — a plugin manifest says the same |
| holds the contract vocabulary | mirrors it, checkably | **owns it** |
| registry of implementations | none | **yes** — plugin + connector conformance in one index |
| resolves `requires secret_store` to an implementation | never | **yes** |
| dispatches a contract call | never | **yes** |

The mirroring is the part that needs care. `Role` is a closed enum *in this repository*
(`ir.rs:501`, with the recorded reason that an open string set is a tag system and a tag system
cannot be checked). If flux owns the registry, the vocabulary exists in two places and can disagree.
That is a real cost, and it is the same cost `connector-pack::DEFAULT_SERVICE`
(`crates/connector-pack/src/credentials.rs:73`) already pays deliberately — with the mitigation
recorded there: *"a mirror is only safe if drift is **checked** rather than promised."* Any contract
vocabulary shared with flux needs the equivalent conformance test, or the closed set stops being
closed at the seam.

### Contracts buy substitutable binding, not just discovery

`provider-roles.md` designs the question. This designs the answer.

**The binding model, in three parts:**

1. **A requirement.** A flow, or an application built on flux, declares `requires secret_store`. It
   names a contract, never a vendor. This is the artifact `provider-roles.md` has no equivalent of.
2. **A binding.** An operator binds `secret_store → 1password` (a connector service) or
   `secret_store → vault` (a plugin). One binding per requirement per deployment. This is operator
   configuration, which is exactly where `vision.md`'s principle 5 says access decisions belong:
   *"nothing grants itself access… access is granted by operator configuration, deliberately."*
3. **A call through the contract's names.** The flow calls `secret_store.get`, and the binding
   resolves it to `onepassword-item-get` or to the plugin's `vault.read`. **The contract's member
   name is the address the caller uses**; the vendor's is an implementation detail.

Point 3 is where the whole design lives or dies, and it is where the prerequisite bites.

## The prerequisite that blocks everything: C-23

A contract can only offer substitutable binding if the member names it dispatches on are
**standardised**. Operation ids in this repository are not standardised, and the story that would
standardise them — [C-23](../stories/C-23-operation-naming-contract.md), "Make operation names a
stable public contract" — is `status: backlog`, with a Progress section reading `(not started)`.

That is not incidental. `fills_slot` (`crates/connector-spec/src/ir.rs:556`) matches **trailing name
segments** precisely *because* there is no naming contract to match against. Its own doc comment
concedes the consequence: *"A one-segment slot is loose, and known to be."*

### Measured, on the catalogue as it is today

`provider-roles.md:133` and [C-121](../stories/C-121-llm-catalogue-role.md)`:71-79` measured this at
19 providers. The catalogue has since more than doubled. Re-measured:

| slot | at 19 providers (`provider-roles.md:133`, `C-121:71-79`) | **today, at 43 providers / 242 operations / 50 services** |
|---|---|---|
| `get` | 37 operations, 17 providers | **77 operations, 38 of 41 providers, 44 of 48 services** |
| `list` (bare) | 9 of 19 providers | **58 operations, 30 of 41 providers, 32 of 48 services** |
| `models.list` | openai, openrouter (+ anthropic) | **3 operations, 3 providers, 3 services — unchanged** |
| `delete` | not measured | 6 operations, 6 providers |
| `put` | not measured | **0 operations, 0 providers** |

**The looseness got worse, not better.** A bare `list` went from 47% of providers to **73%**. `get`
is now filled by 38 of 41 — the only three providers with no `get`-filling operation are
`cloudflare`, `slack`, and `zendesk` (the last being the `show` outlier `provider-roles.md` already
names).

`models.list` held perfectly: exactly three operations, one per vendor, across a catalogue that
doubled. That is the strongest available evidence for C-121's correction — multi-segment slots are
tight, one-segment slots are not, and the fix is the slot's *spelling*.

**The doc comment at `ir.rs:552` is now stale in both the numerator and the denominator.** It says *"a
bare `list` is a suffix nine of seventeen shipped providers contain somewhere"*; it is thirty of
forty-one, and "seventeen" disagreed with `provider-roles.md`'s own "nineteen" even when it was
written.

### The finding that corrects the obvious expectation

The expectation going in was that a `secret_store` contract requiring `get` / `put` / `delete` would
be **spuriously satisfied by a large fraction of the catalogue on day one**. Measured, the truth is
sharper and more useful:

> **`put` is filled by exactly zero operations, so a `secret_store` contract as spelled is satisfied
> by nothing at all — and it would stay unsatisfiable no matter how many CRUD vendors were added.**

No operation id in the catalogue even *contains* the substring `put`
(`jq -r '.providers[].operations[].id' web/public/catalog.json | grep -ci put` → 0). Thirteen
operations use the HTTP method `PUT`, and every one of them is *named* for its domain verb:
`asana-task-update`, `contentful-entry-publish`, `docusign-envelope-void`,
`babelforce-call-session-set`. The catalogue's convention — never written down, which is the problem
— is that a name states what the operation *does*, not which HTTP verb carries it.

So the slot vocabulary fails in **two opposite directions at once**, and both are C-23:

- **Too loose to discriminate.** `get` matches 44 of 48 services. Any contract requiring a bare `get`
  is satisfied by nearly the whole catalogue, which makes the claim worthless.
- **Too narrow to express.** `put` matches nothing, because the catalogue never spells that concept
  that way. Any contract requiring it is satisfied by nothing, which makes the claim unusable.

A vocabulary that is simultaneously over- and under-inclusive is not a vocabulary that needs tuning.
It is the absence of one.

The current shipped role demonstrates it directly. `Role::LlmCatalogue::required_members` returns
`&["list"]` (`crates/connector-spec/src/ir.rs:533-537`) — the *bare* spelling that
`provider-roles.md` and C-121 both identify as wrong, because C-121 is `ready` and has not landed. By
the measurement above, **32 of 48 services would pass that check.** One provider declares the role
(`providers/anthropic.toml:195`) and the loader dutifully verifies it, and the verification is one
that two-thirds of the catalogue would also pass. The check is real; what it checks is nearly
nothing.

**Conclusion: C-23 is a hard prerequisite.** Not a nice-to-have, not parallelisable. A contract is a
promise about names, and this repository has no contract about names.

## Why renaming is not the fix

The reflex is to rename operations into a standard vocabulary. `AGENTS.md:376` forecloses it:

> **An address, once published, is not reused.** Renaming a service or an operation mints a new
> address and deprecates the old one; it never repoints an existing one.

`provider-roles.md` already worked through the arithmetic for one slot and reached the same place:
renaming `jira-issue-get → jira-issue-show` would mint a new address and deprecate one *in order to
make a 17-provider majority match a single outlier*. At 41 providers that is 38 addresses against 1,
and the trade is worse, not better.

C-23 does not change this. C-23 pins how a name is *derived and kept stable*; it cannot retroactively
respell 232 published ids. So the rule stands, and it forces the design:

> **A slot is a set of accepted spellings, not one string.**

That is C-121's correction (`crates/connector-spec/src/ir.rs` already types `required_members` as
`&'static [&'static str]`, so the shape is there and only the set-of-spellings semantics is missing),
and a contract inherits it. A `secret_store` contract's write slot would be spelled something like
`put | set | update | create`, and the union would then match — at the cost that the same union also
matches 23 `create` operations and 14 `update` operations that have nothing to do with secrets.

**Which is the honest statement of the problem: widening the slot to make it satisfiable is the same
move as making it undiscriminating.** There is no spelling of `get`/`put`/`delete` over the current
catalogue that is both. That is what "C-23 is a hard prerequisite" means concretely.

## The two planes, and the seam that does not exist

The word "secret store" already denotes two entirely separate things in this repository, and they
share **no code**.

| | `SecretStore` | `Role` |
|---|---|---|
| where | `crates/connector-secrets/src/lib.rs:97` | `crates/connector-spec/src/ir.rs:501` |
| what it is | a **Rust trait** — `get` / `put` / `delete` over a `CredentialRef` | an **IR declaration** — a closed enum on a `Service` |
| implementations | two: `MemoryStore` (`memory.rs:107`), `VaultStore` (`vault.rs:291`) | one variant: `LlmCatalogue` |
| how it is chosen | **host-injected at construction** — `Arc<dyn SecretStore>` into `Credentials::new` | authored in TOML, checked at load |
| when it is checked | compile time (the trait) | load time (`missing_role_members`, `ir.rs:983`) |
| visible to the other | **invisible to the IR** — no provider TOML can name it | **invisible to Rust** — no runtime type carries it |

The overlap is almost comic: `SecretStore`'s three methods are *literally* `get`, `put` and `delete`
(`lib.rs:106`, `:114`, `:128`) — the exact three slots a `secret_store` contract would require, and
the exact three that the measurement above shows the IR plane cannot express.

**There is no seam between them, and the substitution model needs one.** Concretely, it needs to
answer: when a flow says `requires secret_store` and the operator binds a *connector*, what does the
host construct? Today `Credentials::new` takes an `Arc<dyn SecretStore>` — a Rust object. A connector
is a `ToolSpec` and a parsed declaration. Nothing turns the second into the first.

Sketching what the seam would have to be, without proposing it as work:

- **A contract has a Rust shape and an IR shape, and they are checked against each other.** For
  `secret_store` the Rust shape already exists and is stable; the IR shape would be the slot set. A
  test asserting the two agree — that every method of the trait has a slot, and vice versa — is what
  keeps a mirror honest, the same discipline
  `the_elided_service_is_the_one_the_addressing_reserves` applies to `DEFAULT_SERVICE`.
- **An adapter from a bound connector to the trait.** Given a service holding the contract and a
  `ToolRegistry` that carries its operations, produce an `impl SecretStore` whose `get` dispatches to
  whichever operation fills the `get` slot. That adapter is small and it is the entire mechanism —
  and note that **it belongs in flux**, per the ownership split above, because it needs the registry.
- **A refusal when the shapes disagree.** A connector satisfying the *names* but not the *types* — a
  `get` that takes three required parameters the contract cannot supply — must be refused at bind
  time. `provider-roles.md` already specifies this for roles (*"a required operation whose declared
  parameters cannot satisfy the role's shape is refused"*) and it is unimplemented; a contract makes
  it mandatory rather than nice.

That third bullet is where the member-IO-schema work ([member-io-schemas.md](member-io-schemas.md))
becomes load-bearing rather than merely useful. A contract that checks names and not types is a
contract that binds the wrong thing successfully.

## The bootstrapping problem — stated, not solved

A connector's credential is resolved **through** a `SecretStore`. That is not a convention, it is the
type signature: `Credentials` holds an `Arc<dyn SecretStore>`
(`crates/connector-pack/src/credentials.rs:80-84`, bound at `:103`) and every operation the pack installs resolves
through it, with no global, no `OnceLock` and — deliberately — *"no environment fallback for the
secret itself"* (`credentials.rs:6-11`).

**A connector that *is* the secret store needs a different resolution path.** 1Password's API takes
a service-account token. That token has to come from somewhere, and it cannot come from the store
whose credential it is.

Three shapes the answer could take, none of them designed anywhere in this repository:

- **A bootstrap store**, bound alongside the contract binding, holding only the credentials of
  contract implementations. Turtles, but a finite stack of them.
- **The contract implementation's credential is host configuration**, not tenant configuration —
  which is roughly what `Level::Operator` already distinguishes in
  [connector-configuration.md](connector-configuration.md), and would be an argument that a
  `secret_store` implementation is by construction operator-level.
- **Refuse the cycle at bind time.** A contract implementation may not itself require the contract it
  implements. Cheap to check, and it converts a runtime deadlock into a configuration error.

**Nothing in the repository addresses this**, and it is not a corner case — it is the *first* thing
that happens when someone binds the motivating example. It should be answered before a `secret_store`
contract is defined, not after.

## `aws.s3` as a standalone connector — framed, not decided

The owner named `aws.s3`. The charter question it raises is genuine and this document does not settle
it; it states what the boundary actually says so the decision can be made deliberately.

**The case that S3 is a service.** It is HTTP with an OpenAPI-describable surface. Amazon bills for
it. `provider-services.md` already uses `s3` and `bedrock-runtime` under an AWS provider as *the*
motivating example for the whole service level, and `Service::base_url`'s doc comment
(`crates/connector-spec/src/ir.rs:582-586`) names them by hand: *"AWS is the motivating case:
`s3.amazonaws.com` and `bedrock-runtime.<region>.amazonaws.com` share an authority and not a host."*
The IR was shaped around this vendor.

**The case that S3 is a technology.** Nobody integrates *with* S3 the way they integrate with
Zendesk; they *store objects in* it, the way they store rows in Postgres. It is stateful, its access
model is a policy language, and its ecosystem assumes multipart upload, presigning and streaming —
none of which is one-request-one-response. Most decisively for this repository:
**SigV4 request signing is not expressible in the current auth model.** `unified-auth.md`'s three
axes are source × acquisition × placement, and signing a canonical request over its own method, path,
headers and body payload hash is none of those. `AGENTS.md`'s authentication contract already names
`Signing` as *"the one deliberate divergence"* — for **inbound webhook verification**, a secret that
never goes out. SigV4 is the mirror case and has no home.

**The boundary already admits it cannot classify this.**
[C-46](../stories/C-46-generic-connectors.md) concedes exactly this, in its own words:

> `AGENTS.md` currently says: *"Connectors are paid SaaS services"* and *"technology adapters stay in
> flux as plugins"*. A generic `http` or `mcp` connector is neither a paid SaaS service nor a
> technology adapter — it is a **protocol** connector, **a third category the boundary does not
> name**.

S3 is a fourth: a **paid SaaS service that behaves like a technology**. It satisfies the charter's
stated test (paid, SaaS, HTTP) and fails its evident intent (stateful, protocol-rich, "real Rust
earns its keep"). That the two criteria disagree on a case this prominent is a defect in the
boundary, not in S3.

**What this document says: do not decide it here, and do not decide it as a side effect.** Two
questions are tangled and separating them is the useful contribution:

1. **Charter** — does a paid SaaS service that behaves like a technology belong here? That is a
   `vision.md` amendment and it is C-46's question, generalised. It is unaffected by (2).
2. **Technical** — *can* a SigV4-signed connector be emitted at all? Independent of (1), and probably
   answered "no, not without a new auth axis". C-46 draws exactly this distinction for `mysql`
   (*"blocked on a missing primitive, not impossible"*), and the same split applies.

A `secret_store` contract does not need S3 and should not wait for it. Bundling them would let a
capability-contract design silently decide a charter question, which is the failure mode C-34 was
filed to prevent for the proxy.

## Out of scope

- **Defining any concrete contract.** Not `secret_store`, not anything else. C-23 first; a contract
  defined over an unstandardised namespace is a promise about nothing.
- **A `providers/vault.toml`.** Stated at the top and repeated here because it is the misread this
  document most needs to survive: Vault satisfies a contract **as a plugin**, and `AGENTS.md:49-50`
  is unchanged.
- **The registry, the resolver and the dispatcher.** They live in flux. This repository declares
  conformance and nothing more.
- **A contract hierarchy.** `provider-roles.md` chose closed and flat for roles, on the reasoning
  that widening later is cheap and narrowing an open system is not. A contract inherits that until
  something concrete demands otherwise.
- **Deciding `aws.s3`.** Framed above; C-46 owns the charter half.
