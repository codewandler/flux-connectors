// The shape of the generated catalogue, and every question the explorer asks of it.
//
// The types mirror `docs/designs/catalog-json.md`, which is the contract: every key is always
// present, an absent value is `null` or `[]`. Nothing here invents a value — no provider, no
// operation and no issue code is named in this file or in any component. Add a fourth provider to
// `providers/*.toml` and the site covers it with no edit.

/** `catalog` | `provider` | `operation` — how far a condition reaches. */
export type Scope = 'catalog' | 'provider' | 'operation'

export interface Issue {
  code: string
  scope: Scope
  summary: string
  params: string[]
}

export interface Status {
  works: boolean
  issues: Issue[]
}

export interface Parameter {
  name: string
  in: string
  wire: string | null
  description: string
  required: boolean
  schema: Record<string, unknown>
}

/**
 * One API surface of a provider — the unit that is addressed, versioned, selected and installed.
 *
 * A service owns its own `base_url` and `api_version`, defaulting to its provider's, and `gid` is the
 * address a consumer copies. Both `api_version` and `gid` are `null` for a provider that declares
 * neither, which is most of them.
 */
export interface Service {
  name: string
  description: string
  base_url: string
  hosts: string[]
  api_version: string | null
  gid: string | null
  operation_count: number
}

export interface Operation {
  id: string
  provider: string
  /** The service this operation belongs to — exactly one, and every operation has one. */
  service: string
  description: string
  risk: string
  idempotency: string
  method: string
  path: string
  parameters: Parameter[]
  body_schema: Record<string, unknown> | null
  response_schema: Record<string, unknown> | null
  credentials: string[][]
  hosts: string[]
  flux: string
  status: Status
}

export interface Credential {
  name: string
  scheme: { kind: string; name: string | null }
  description: string
  env: string[]
  user_env: string[]
  user_suffix: string | null
  oauth2: boolean
}

export interface Auth {
  schemes: string[]
  credentials: Credential[]
  default: string[][]
}

export interface Provider {
  id: string
  authority: string | null
  vendor: string
  description: string
  base_url: string
  api_version: string | null
  hosts: string[]
  services: Service[]
  auth: Auth
  operation_count: number
  operations: Operation[]
}

export type CoreAvailability = 'available' | 'planned'

export interface ToolSpec {
  name: string
  description: string
  input_schema: Record<string, unknown>
  effects: string[]
  risk: string
  idempotency: string
  access: string[]
  group?: string
}

export interface CoreEntryBase {
  $schema: string
  $id: string
  schema_version: number
  name: string
  title: string
  description: string
  category: string[]
  availability: CoreAvailability
}

export interface CoreOperation extends CoreEntryBase {
  kind: 'operation'
  tool_spec: ToolSpec
}

export interface CoreNode extends CoreEntryBase {
  kind: 'node'
  schema_ref: string
}

export interface CoreCapability extends CoreEntryBase {
  kind: 'capability'
  callable: boolean
  operation_ids: string[]
}

export type CoreEntry = CoreOperation | CoreNode | CoreCapability

export interface CoreSchemas {
  catalog: Record<string, unknown>
  entry: Record<string, unknown>
  flux_ast: Record<string, unknown>
}

export interface CoreCatalog {
  $schema: string
  $id: string
  schema_version: number
  generator: string
  operations: CoreOperation[]
  nodes: CoreNode[]
  capabilities: CoreCapability[]
  schemas: CoreSchemas
}

export interface Catalog {
  schema_version: number
  generator: string
  providers: Provider[]
  core: CoreCatalog | null
}

/** Every Flux-owned core entry in its declared kind order. */
export function allCoreEntries(core: CoreCatalog): CoreEntry[] {
  return [...core.operations, ...core.nodes, ...core.capabilities]
}

/** The stable explorer page for a Flux-owned core entry. */
export function coreEntryHref(entry: CoreEntry): string {
  const section = entry.kind === 'capability' ? 'capabilities' : `${entry.kind}s`
  return `/core/${section}/${encodeURIComponent(entry.name)}`
}

/** Resolve a canonical core specification id to the entry that owns it. */
export function coreEntryById(core: CoreCatalog, id: string): CoreEntry | undefined {
  return allCoreEntries(core).find((entry) => entry.$id === id)
}

/**
 * **The distinction the whole explorer turns on.**
 *
 * `works` is `false` for all 25 operations today and that is correct — no provider can make a live
 * call yet, because the auth seam has not landed in flux. Rendering that as "0 of 25 working" would
 * be accurate and useless, and it would tar 20 operations that are working exactly as designed.
 *
 * `scope` is what separates the two: an issue an operation *owns* is a defect in that operation, and
 * an issue scoped to its provider or to the catalogue is a condition it merely inherits along with
 * everything else. The first is a badge on the operation; the second is a banner over the set.
 */
export function ownIssues(operation: Operation): Issue[] {
  return operation.status.issues.filter((issue) => issue.scope === 'operation')
}

/** The conditions an operation inherits from its provider or from the catalogue as a whole. */
export function inheritedIssues(operation: Operation): Issue[] {
  return operation.status.issues.filter((issue) => issue.scope !== 'operation')
}

/** Whether this operation has a problem nothing else in the catalogue has. */
export function ownsDefect(operation: Operation): boolean {
  return ownIssues(operation).length > 0
}

/** Every operation, flattened across providers, in the order the catalogue declares them. */
export function allOperations(catalog: Catalog): Operation[] {
  return catalog.providers.flatMap((provider) => provider.operations)
}

/** One issue per `code`, keeping the first occurrence — a shared condition is stated once. */
export function distinct(issues: Issue[]): Issue[] {
  const seen = new Map<string, Issue>()
  for (const issue of issues) if (!seen.has(issue.code)) seen.set(issue.code, issue)
  return [...seen.values()]
}

/** The conditions that reach the whole catalogue, stated once. */
export function catalogIssues(catalog: Catalog): Issue[] {
  return distinct(
    allOperations(catalog)
      .flatMap((operation) => operation.status.issues)
      .filter((issue) => issue.scope === 'catalog')
  )
}

/** The conditions that reach every operation of one provider, stated once. */
export function providerIssues(provider: Provider): Issue[] {
  return distinct(
    provider.operations
      .flatMap((operation) => operation.status.issues)
      .filter((issue) => issue.scope === 'provider')
  )
}

/** How many operations own a defect — the only headline count worth showing. */
export function defectCount(operations: Operation[]): number {
  return operations.filter(ownsDefect).length
}

/**
 * The operation's signature, taken from the first line of its generated Flux.
 *
 * Reassembling `name(param: Type, …) -> Any` from the parameter list would be a second, subtly
 * different renderer for something the emitter already decided. This is the emitter's own answer.
 */
export function signature(operation: Operation): string {
  return operation.flux.split('\n', 1)[0]
}

/** The stable, deep-linkable URL of one operation, relative to the site root. */
export function operationHref(operation: Operation): string {
  return `/operations/${operation.id}`
}

/**
 * A short label for a parameter's type, from the vendor's JSON Schema.
 *
 * A label only — the schema itself is always shown verbatim alongside it, because the constraint
 * keywords a label drops (`format`, `enum`, `minimum`) are often the ones a caller needs.
 */
export function schemaType(schema: Record<string, unknown>): string {
  const alternatives = (schema.oneOf ?? schema.anyOf) as Record<string, unknown>[] | undefined
  if (Array.isArray(alternatives)) return alternatives.map(schemaType).join(' | ')

  const type = schema.type
  if (Array.isArray(type)) return type.join(' | ')
  if (typeof type === 'string') {
    if (type === 'array') {
      const items = schema.items as Record<string, unknown> | undefined
      return items ? `array<${schemaType(items)}>` : 'array'
    }
    return type
  }
  return 'any'
}

/** The distinct values of one operation facet, in the order the catalogue declares them. */
export function facet(operations: Operation[], pick: (operation: Operation) => string): string[] {
  return [...new Set(operations.map(pick))]
}

/**
 * The reserved service name — the only name in this file that is not read out of the catalogue,
 * because it is not catalogue data.
 *
 * It is vocabulary from the address grammar: an operation naming no service belongs to it, no
 * provider may declare it, and it is elided from every published address and every file name. The
 * consequence this site has to honour is that it is never rendered — a card listing it, or a filter
 * offering it, would name something no address contains.
 */
const RESERVED_SERVICE = 'default'

/** The services a provider publishes under a name of their own, in catalogue order. */
export function namedServices(provider: Provider): Service[] {
  return provider.services.filter((service) => service.name !== RESERVED_SERVICE)
}

/**
 * The service options a visitor can choose from, narrowed to one connector when one is chosen.
 *
 * Dependent in one direction only, which is the obvious one: choosing a connector narrows the
 * services to that connector's, and choosing a service with no connector chosen stays valid. A
 * connector that addresses a single surface offers nothing here — its one service is the reserved
 * one, and it names no address to filter on.
 */
export function serviceFacet(providers: Provider[], provider = ''): string[] {
  const scope = provider ? providers.filter((owner) => owner.id === provider) : providers
  return [...new Set(scope.flatMap((owner) => namedServices(owner).map((service) => service.name)))]
}

/**
 * The service an operation should state, or `null` when its connector addresses a single surface.
 *
 * Naming the service of a connector that has only one would repeat what its card already says, and
 * for the reserved one it would name something the address elides.
 */
export function operationService(provider: Provider, operation: Operation): string | null {
  return namedServices(provider).length > 1 ? operation.service : null
}

/**
 * The `api_version` worth showing for a service: `null` when it is its connector's own.
 *
 * A service inherits its connector's version unless it overrides it, so repeating the inherited
 * value on every service would bury the one that actually differs.
 */
export function serviceApiVersion(provider: Provider, service: Service): string | null {
  return service.api_version === provider.api_version ? null : service.api_version
}

/**
 * The address a consumer copies for a connector that addresses a single surface, or `null`.
 *
 * `gid` is a property of a service, and for such a connector that service is the reserved one —
 * whose name the address elides, which is exactly why the value reads as the connector's own rather
 * than as a service's. It is `null` for every connector that declares no authority, and a null
 * address renders as nothing at all: a placeholder would put a value on the page that the catalogue
 * does not publish and no consumer could copy.
 */
export function providerAddress(provider: Provider): string | null {
  const reserved = provider.services.find((service) => service.name === RESERVED_SERVICE)
  return reserved?.gid ?? null
}

// ---------------------------------------------------------------------------------------------
// The shareable view.
//
// The explorer promises a stable page per operation. A *view* — "every destructive operation of one
// connector" — had no address at all, because filtering was component state. Putting that state in
// the query string is what makes the promise true of a view, and it is here rather than in the
// component because a serialiser is exactly the kind of thing that should be provable by a test.
// ---------------------------------------------------------------------------------------------

/**
 * The complete filter and sort state of the operation list.
 *
 * Every field is a string, and the empty string means *unset* — not "any" as a magic value, but the
 * absence of a constraint, which is what a missing query parameter means too. That correspondence is
 * why the pair below is as short as it is.
 */
export interface View {
  query: string
  provider: string
  service: string
  risk: string
  idempotency: string
  defect: string
  sort: string
}

/**
 * The orders the list can be shown in.
 *
 * `catalog` is the default and it is not a fallback: it is the order the emitter writes operations
 * into the generated module, so it is the order a reader of the Flux sees. The other two are the
 * re-orderings worth offering over a list this long.
 */
export const SORTS = ['catalog', 'id', 'risk']

/**
 * Risk tiers from least to most consequential.
 *
 * The one ordering the catalogue cannot supply — JSON carries the tier of each operation and no
 * notion of which tier is worse. It has to be declared, and declaring it is the whole point:
 * alphabetically the most consequential tier sorts second of four, which is wrong without ever
 * looking wrong.
 */
export const RISK_ORDER = ['low', 'medium', 'high', 'destructive']

/** The choices the defect filter offers, which are vocabulary of this site and not catalogue data. */
const DEFECTS = ['own', 'none']

/**
 * The query-string key for each field, in the order they are written.
 *
 * Both directions read this one table, so a key cannot be spelled one way when written and another
 * way when read, and the order here is what makes the encoding canonical. The keys are what a
 * visitor sees on the controls rather than what the code calls them — a URL is read and edited by
 * people, and the control is called *Connector*.
 */
const VIEW_KEYS: [keyof View, string][] = [
  ['query', 'q'],
  ['provider', 'connector'],
  ['service', 'service'],
  ['risk', 'risk'],
  ['idempotency', 'idempotency'],
  ['defect', 'defect'],
  ['sort', 'sort'],
]

/** The unfiltered view: no constraint anywhere, and the catalogue's own order. */
export function emptyView(): View {
  return {
    query: '',
    provider: '',
    service: '',
    risk: '',
    idempotency: '',
    defect: '',
    sort: SORTS[0],
  }
}

/**
 * A view as a query string, without the leading `?`.
 *
 * An unset field contributes no parameter and the default sort is an unset field, so the unfiltered
 * view is the empty string and the unfiltered URL is clean. Two routes to the same view produce the
 * same string, because the key order is the table's and not the caller's.
 */
export function encodeView(view: View): string {
  const unset = emptyView()
  const params = new URLSearchParams()
  for (const [field, key] of VIEW_KEYS) {
    const value = String(view[field] ?? '').trim()
    if (value && value !== unset[field]) params.set(key, value)
  }
  return params.toString()
}

/**
 * The view a query string names, with anything unrecognised ignored rather than fatal.
 *
 * A link outlives the page it was copied from, so a parameter this build has never heard of is
 * dropped and a field is only accepted when its value is one this file owns the vocabulary for —
 * the sort and the defect filter. The catalogue's own vocabularies (connector, service, risk,
 * idempotency) cannot be checked here, because a pure parse has no catalogue; `narrowView` is the
 * other half.
 */
export function decodeView(search: string): View {
  const params = new URLSearchParams(search)
  const view = emptyView()

  for (const [field, key] of VIEW_KEYS) {
    const value = (params.get(key) ?? '').trim()
    if (value) view[field] = value
  }

  if (!SORTS.includes(view.sort)) view.sort = SORTS[0]
  if (view.defect && !DEFECTS.includes(view.defect)) view.defect = ''

  return view
}

/**
 * The view narrowed to what this catalogue actually offers.
 *
 * The second half of *ignored, not fatal*. A shared link naming a connector that has since been
 * renamed, or a service its connector no longer publishes, must degrade to a **wider** view — a
 * value nothing can match would render an empty catalogue and read as an outage. Dropping the
 * constraint shows more than was asked for, which is the honest failure of the two.
 *
 * It also carries the service filter's one dependency: a service is only kept while the chosen
 * connector still publishes it, so changing connector drops a service that connector never had.
 */
export function narrowView(view: View, providers: Provider[]): View {
  const operations = providers.flatMap((owner) => owner.operations)
  const offered = (value: string, options: string[]) => (options.includes(value) ? value : '')

  const provider = offered(view.provider, providers.map((owner) => owner.id))

  return {
    ...view,
    provider,
    service: offered(view.service, serviceFacet(providers, provider)),
    risk: offered(view.risk, facet(operations, (operation) => operation.risk)),
    idempotency: offered(view.idempotency, facet(operations, (operation) => operation.idempotency)),
  }
}

/** Where a risk tier ranks, with a tier this build has not heard of ranked after every one it has. */
function riskRank(operation: Operation): number {
  const rank = RISK_ORDER.indexOf(operation.risk)
  return rank === -1 ? RISK_ORDER.length : rank
}

/**
 * How two operations compare under one sort.
 *
 * The default compares everything as equal, which is not a stub: `Array.prototype.sort` is stable,
 * so a comparator that separates nothing leaves the catalogue's own order exactly as it was. The
 * same stability is what makes catalogue order the tiebreaker inside a risk tier, so choosing risk
 * groups the list without discarding the order underneath it.
 *
 * Ids are compared by code point rather than by locale: the site is one document and the order it
 * shows must not depend on the reader's browser.
 */
export function compareOperations(sort: string): (a: Operation, b: Operation) => number {
  if (sort === 'id') return (a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0)
  if (sort === 'risk') return (a, b) => riskRank(a) - riskRank(b)
  return () => 0
}

/** The operations in one sort's order, as a new list — the caller's own is left alone. */
export function sortOperations(operations: Operation[], sort: string): Operation[] {
  return [...operations].sort(compareOperations(sort))
}
