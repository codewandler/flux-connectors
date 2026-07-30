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
  story: string
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

export interface Operation {
  id: string
  provider: string
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
  vendor: string
  description: string
  base_url: string
  hosts: string[]
  auth: Auth
  operation_count: number
  operations: Operation[]
}

export interface Catalog {
  schema_version: number
  generator: string
  documentation: string
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
