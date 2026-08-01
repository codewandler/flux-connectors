# The explorer's components, in three tiers

These fourteen Vue components are the explorer. Since C-142 **none of them imports VitePress**, so
the set can be mounted somewhere other than this site — a
product's own admin surface, a Storybook, a test harness — without a rewrite and without extracting a
package.

That is not an aspiration; `test/explorer.test.mjs` asserts it, in
`no_component_imports_the_site_framework`. A component may import **Vue**, a **sibling component**,
and **`data/catalog.mts`**. Nothing else. In particular it may not import `node:*` or the build-time
loader `data/catalog.data.mts`: a component that reaches for its own data cannot be attached
anywhere, so everything a component renders arrives as a prop or as injected context.

## The one thing a host has to supply

`data/catalog.mts` answers *which page* — `/operations/<id>`, `/core/<section>/<name>` — and that
answer is the catalogue's, identical wherever the components are mounted. Turning that path into an
href a browser can follow is the **host's** answer, and it differs: this site is served under a base
path and so has `withBase`; another host has its own router, or none at all.

So it is a port, not an import:

```ts
import { inject } from 'vue'
import { PATH_RESOLVER, identityPath, type PathResolver } from '../../../data/catalog.mts'

const resolvePath = inject<PathResolver>(PATH_RESOLVER, identityPath)
```

The default is **identity** — a host that says nothing leaves the path exactly as the catalogue gave
it, which is the honest behaviour and not a fallback that quietly breaks links. This site supplies
`withBase` from [`../index.mts`](../index.mts), which is the only file under `.vitepress/theme/` that
knows the framework's name.

## The tiers

### Presentational — props only, no catalogue knowledge

Renders what it is given. Its props are strings, numbers and plain objects; it could be lifted into
any Vue application that has never heard of this catalogue.

| Component | Takes |
|---|---|
| `FluxSource.vue` | `source: string` |
| `SchemaBlock.vue` | `schema: unknown` |
| `SpecChip.vue` | `value: string` |

`FluxSource` deliberately does **not** highlight. Shiki has no Flux grammar, and colouring Flux by
another language's rules would be worse than plain text — so it is plain text, the bytes the emitter
produced. Do not "fix" this; `SchemaBlock` highlights because JSON and YAML are real grammars.

### Catalogue-aware — typed against `data/catalog.mts`

Knows the *shape* of the catalogue and none of its contents. It still takes everything it renders as
a prop; the coupling is to the type, not to a source of data.

| Component | Takes |
|---|---|
| `IssueNotice.vue` | `issues: Issue[]` |
| `ParameterTable.vue` | `parameters: Parameter[]` |
| `StatusBadge.vue` | `operation: Operation` |
| `ProviderCard.vue` | `provider: Provider` |
| `OperationRow.vue` | `operation: Operation` (+ `resolvePath`) |
| `CoreExplorer.vue` | `core: CoreCatalog` (+ `resolvePath`) |
| `CatalogSnapshot.vue` | `catalog: Catalog` (+ `resolvePath`) |

`CoreExplorer` holds local filter state. That is ephemeral view state, not routing — it is not in the
URL and nothing outside the component can observe it — so it stays in this tier.

### Page — owns routing and state

Mounted by a page, addressed by a URL, and the tier where a route parameter or the query string is
allowed to matter. These are what `../index.mts` registers globally.

| Component | Mounted by | Owns |
|---|---|---|
| `CatalogExplorer.vue` | `explorer.md` | the explorer's composition and its headline counts |
| `OperationDetail.vue` | `operations/[operation].md` | resolving the `id` route parameter against the catalogue |
| `CoreDetail.vue` | `core/[kind]/[name].md` | resolving the `kind`/`name` route parameters |
| `OperationList.vue` | `CatalogExplorer.vue` | the shareable view: the query string, read on mount and **replaced**, never pushed |

Two rules this tier exists to hold:

- **Read the URL on mount, not during setup.** There is no `location` while the page is being
  rendered, and reading one during setup would make the server's markup disagree with the client's
  first render. `OperationList` guards with a local `typeof window !== 'undefined'` rather than
  importing the framework's `inBrowser`.
- **Replace, never push.** A pushed history entry per filter change means the back button walks back
  through every keystroke of a search instead of leaving the explorer.

## What a component may say about a field it was not given

Since C-408 the components are mounted over catalogues this repository does not generate, and such a
source may publish a **thinner** document — no `auth`, no `credentials`, no `method`/`path`, no
`flux`, no `base_url`. Every one of those absences used to render as a statement about the
**connector**: a red "not configured" on every card, "live calls are disabled" on every operation,
two empty chips.

So `[]` and *absent* are no longer the same thing. `data/catalog.mts` types such a field
`Published<T>`, and `published(value)` is the only place an absence is read:

```ts
const auth = computed(() => providerAuth(props.provider))
```

Three outcomes, and the middle one is the reason this is not a softening:

| the document says | it means | it renders |
|---|---|---|
| `auth.schemes` non-empty | the connector authenticates with these | the schemes |
| `auth.schemes` empty | the connector declares none | **"not configured"**, in `--vp-c-danger-1` |
| no `auth` at all | *this source does not publish auth* | `UNPUBLISHED`, muted |

A withheld credential is a safety property worth showing in red and **must keep rendering as one**;
"this catalogue does not carry that field" is a statement about the document and is muted. Do not
merge the two branches back together to make a template shorter — that is the bug, not the tidy.

Note what this deliberately is *not*: no component learns which source it is rendering. The
distinction is a property of the document, so it arrives with the data that was already there — no
new prop, no new injection, and the import rule above is untouched.

## And the rule that outranks all three

**No hand-written catalogue data, in any tier.** No component names a provider, a vendor, a service,
a host, a credential, an operation id or an issue code. The last tests in `test/explorer.test.mjs`
enforce this mechanically against the generated `public/catalog.json`. A "reusable" component that
hardcoded one of those would trade the discipline the whole catalogue depends on for a convenience,
which is the exact failure this repository exists to correct.
