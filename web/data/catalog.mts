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

export interface Catalog {
  schema_version: number
  generator: string
  providers: Provider[]
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
