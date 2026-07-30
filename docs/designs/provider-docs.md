# Design: rendered provider documentation

**Status:** proposed · **Pillar:** Codegen · **Stories:** C-31 … C-33

## Why

A connector's operations are currently legible only by reading `providers/<name>.toml` or the
generated `.flux`. Neither is documentation: the TOML is compiler input and the Flux is compiler
output, and both omit the one thing a person actually wants — *how do I call this, and what comes
back?*

Everything needed is already in the IR: the operation set, typed parameters with their JSON Schema,
descriptions, risk, idempotency, and the credential each operation requires. A **third emitter**,
alongside the Flux module and the connector manifest, renders that into one markdown page per
provider — so the documentation is generated from the same source as the code and cannot drift from
it.

Per operation the page shows the same call in more than one form, as sibling tabs:

- **`<operation>.flux`** — what the connector actually generated, so a reader can see the real thing.
- **`curl`** — the same request as raw HTTP, which is how most people sanity-check an integration.

The two tabs are the point. A Flux fence alone assumes the reader already runs flux; a curl fence
alone hides what the connector does. Together they document the operation and the connector in one
artifact.

## Approach

### A third emitter, same rules as the first two

`connector-docs` (or a module inside `connector-flux`) takes the IR and returns markdown text. It
inherits the properties the existing emitters already have and that the CLI's guarantees rest on:
**deterministic**, **total**, and **returns text rather than writing** — `connector-cli`'s
byte-identical-no-op and atomic-write behaviour depends on all three.

The output is a committed artifact reviewed as a diff, exactly like `<provider>.flux`, and covered by
`flux-connectors check` so stale documentation is a build failure rather than a slow lie.

### Page shape

One page per provider: a header carrying the vendor, base URL and the credentials the connector
needs (names only — never values), then one section per operation with its signature, description,
a parameter table derived from the JSON Schema, and the tabbed fences.

### The tab mechanism is a real decision, not a detail

**Markdown has no standard tab syntax.** The three plausible targets are mutually incompatible:

| Target | Syntax | Renders where |
|---|---|---|
| MkDocs Material | `=== "Flux"` + indented fence | MkDocs only |
| Docusaurus | `<Tabs>` / `<TabItem>` JSX | Docusaurus only |
| Plain CommonMark | sequential fences under `####` headings | everywhere, no tabs |

Plain fences render correctly in every viewer including GitHub and a plain editor; the tabbed forms
look wrong as literal text wherever their renderer is absent. **Recommendation: emit plain
CommonMark by default and make the tabbed dialects opt-in**, so the artifact is readable in the repo
— which is where these pages will mostly be read — without foreclosing a docs site later. C-31 owns
the decision.

### The curl tab has a credential problem, and it decides the epic's scope

A working curl needs a credential. The connector deliberately never holds one. So the curl tab can
be rendered in one of two ways:

1. **Against the vendor, with a placeholder** — `-H "Authorization: Bearer $BABELFORCE_TOKEN"`,
   naming the env var the manifest declares. Honest, copy-pasteable once the reader exports the
   variable, and depends on nothing.
2. **Against a credential-injecting proxy** — no secret in the command at all.

**This epic assumes (1)** so it is independent and shippable now. (2) is the
[connectors-proxy](connectors-proxy.md) epic; if that lands, the curl tab gains a second variant
rather than changing shape.

## Alternatives considered

- **Hand-written provider docs.** Rejected on the repo's founding argument: hand-maintained
  integration material drifts from the API silently and permanently. That is precisely the
  action-proxy failure this project exists to correct.
- **Rustdoc / a docs site as the primary artifact.** Heavier, and it puts the documentation somewhere
  other than next to the connector it describes.
- **Emitting docs from the provider TOML rather than the IR.** The TOML is one of *two* front-ends —
  a spec-ingested connector has no hand-written TOML to render from. The IR is the only place both
  converge.

## Risks & open questions

- **The tab dialect decision is hard to reverse** once pages are committed and linked.
- **Response shapes are not modelled well enough to document yet.** `Operation::response_schema`
  exists but nothing populates it richly; a "Response" section would be mostly empty. Deliberately
  out of scope for the first cut.
- **Doc pages will churn on every codegen change**, because the Flux tab embeds generated output.
  That is correct — it is what keeps them honest — but it makes codegen diffs larger.
- **Credential names in a public repo.** Names, not values, so this is not a leak; worth stating
  explicitly so nobody "fixes" it by removing them.

## Acceptance / done

- `flux-connectors build` emits one markdown page per provider, deterministically.
- Each operation shows its Flux and its curl form, with parameters documented from their schema.
- `flux-connectors check` fails on a stale page.
- No credential value appears on any page.
