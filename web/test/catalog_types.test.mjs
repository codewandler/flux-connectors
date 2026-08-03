// The site's TypeScript types agree with the document they describe (C-158).
//
// `data/catalog.mts` restates the generated catalogue's shape a third time — after the IR and the
// emitter — and one language out, where nothing in either gate could reach it. C-151 had just fixed
// this class twice on the Rust side by *deriving* the field list from `deny_unknown_fields`' own
// error; TypeScript erases at run time and offers no equivalent, so an interface that omits a
// published key compiles, builds, renders and says nothing. `catalog-json.md` makes the drift
// permanent rather than transient: **adding a field does not bump `schema_version`**, precisely
// because "every consumer reads by name, so a new key is invisible to one that does not know it".
// The site is such a consumer, and until this file nothing told it.
//
// **Why an agreement test and not generation.** Generating `catalog.mts` from the emitter would end
// the class rather than test it, and it is the right long-term answer — but the generator would have
// to write into `web/`, which makes the types a *whole-catalogue artifact* under the rule in
// `catalog-json.md`: written by a full build only, owned by the coordinator, and off-limits to a
// scoped provider run. That is a pipeline decision, not a test. It also cannot be had cheaply here:
// `Published<T>` (C-408), the prose on every field, and the site's own vocabulary live in this file
// and are not derivable from the document. An agreement test keeps the file hand-written and takes
// away only the ability to be wrong about it. Worth a follow-up story, not this one.
//
// **How it reads the types.** The declarations are parsed out of `data/catalog.mts` as text. A
// TypeScript parser would be a second dependency and the site has exactly one by design — the same
// trade `ci_gate.test.mjs` makes for YAML and `SchemaBlock.vue` makes for syntax highlighting. The
// grammar actually needed is small: `export interface X extends Y { field?: Type }`.
//
// **How it finds what to check.** Not from a table of paths — that would be a fourth hand-maintained
// restatement of the same shape, with the same failure. The interfaces already say what contains
// what (`Catalog.providers: Provider[]`, `Channel.verification: Verification`), so the walk starts
// at `Catalog` and the document root and descends wherever a declared field's type names another
// interface. An interface added to that graph is covered with no edit here, which is the property
// C-151 bought on the Rust side.
//
// Nothing in this file names a provider, an operation, a credential or an issue code, and it never
// can: it reads key *names* out of the document and type *names* out of the source, and has no
// vocabulary of its own beyond the interfaces the story named.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const catalogPath = path.join(webRoot, 'public', 'catalog.json')
const typesPath = path.join(webRoot, 'data', 'catalog.mts')

/** The type the document's root is declared as — the one name the walk has to be given. */
const ROOT = 'Catalog'

/**
 * The interfaces the story requires this check to actually exercise.
 *
 * A vacuity guard, and the only list here: the walk is driven by the document, so an entity the
 * catalogue happens to carry none of today would be checked zero times and pass in silence. These
 * are type names from `catalog.mts`, never catalogue values — the document decides what is in them.
 */
const MUST_BE_EXERCISED = [
  'Provider',
  'ConfigField',
  'ConfigChoices',
  'Choice',
  'Service',
  'SpecSource',
  'Operation',
  'Channel',
  'SocketConnect',
  'ChannelAuthRequirement',
  'InboundEvent',
  'Verification',
  'Hmac',
  'Reply',
]

/** The generated catalogue the site ships. */
function catalog() {
  assert.ok(
    existsSync(catalogPath),
    `web/public/catalog.json is missing — the site has no catalogue to read. Run \`cargo run -p connector-cli -- build\``
  )
  return JSON.parse(readFileSync(catalogPath, 'utf-8'))
}

/**
 * Comments removed, string literals kept.
 *
 * String-aware rather than a line sweep, for the same reason the guard in `explorer.test.mjs` is:
 * a `'https://…'` in a literal type would be cut at its own `//` and the rest of the file read as
 * comment. It runs before anything looks for a brace, which is what keeps a JSDoc `{@link X}` out
 * of the depth count.
 */
function withoutComments(source) {
  let kept = ''
  for (let i = 0; i < source.length; ) {
    const pair = source.slice(i, i + 2)
    if (pair === '//') {
      const end = source.indexOf('\n', i)
      i = end === -1 ? source.length : end
      kept += ' '
    } else if (pair === '/*') {
      const end = source.indexOf('*/', i + 2)
      i = end === -1 ? source.length : end + 2
      kept += ' '
    } else if (source[i] === "'" || source[i] === '"' || source[i] === '`') {
      const quote = source[i]
      let j = i + 1
      while (j < source.length && source[j] !== quote) j += source[j] === '\\' ? 2 : 1
      kept += source.slice(i, Math.min(j + 1, source.length))
      i = j + 1
    } else {
      kept += source[i]
      i += 1
    }
  }
  return kept
}

/** The text between the brace at `open` and the one that closes it. */
function balanced(source, open) {
  let depth = 0
  for (let i = open; i < source.length; i += 1) {
    if (source[i] === '{') depth += 1
    else if (source[i] === '}') {
      depth -= 1
      if (depth === 0) return source.slice(open + 1, i)
    }
  }
  assert.fail(`an interface body in ${path.basename(typesPath)} is never closed`)
}

/**
 * The fields declared directly in one interface body: name, whether it is optional, and its type as
 * written.
 *
 * Only depth 0 counts, so an inline object type contributes its own field and not the ones nested
 * inside it. `?` is the one modifier that changes the meaning of a *key*, and it is the only one
 * carried through.
 */
function fieldsOf(body) {
  const fields = new Map()
  let depth = 0
  for (const line of body.split('\n')) {
    if (depth === 0) {
      const match = line.match(/^\s*(?:readonly\s+)?([A-Za-z_$][\w$]*)(\?)?\s*:\s*(.+?)\s*$/)
      if (match) {
        const [, name, optional, declared] = match
        fields.set(name, { optional: optional === '?', type: declared.replace(/[;,]\s*$/, '') })
      }
    }
    for (const character of line) {
      if (character === '{') depth += 1
      else if (character === '}') depth -= 1
    }
  }
  return fields
}

/**
 * Every `export interface` in the source, with `extends` flattened into the child.
 *
 * Flattened rather than followed at lookup time because the document does not know it inherited
 * anything: a core entry carries its base's keys in the same object as its own, so the check needs
 * one flat set per interface.
 */
function interfaces(source) {
  const clean = withoutComments(source)
  const declared = new Map()
  const extended = new Map()

  const header = /\bexport\s+interface\s+([A-Za-z_$][\w$]*)\s*(?:extends\s+([A-Za-z_$][\w$]*)\s*)?\{/g
  for (const match of clean.matchAll(header)) {
    const open = match.index + match[0].length - 1
    declared.set(match[1], fieldsOf(balanced(clean, open)))
    if (match[2]) extended.set(match[1], match[2])
  }

  for (const [child, parent] of extended) {
    assert.ok(declared.has(parent), `\`${child}\` extends \`${parent}\`, which is declared nowhere`)
    for (const [name, field] of declared.get(parent)) {
      if (!declared.get(child).has(name)) declared.get(child).set(name, field)
    }
  }
  return declared
}

/**
 * What a field's declared type says the walk should check its value against: another interface, an
 * inline object shape, or nothing.
 *
 * Array-ness, `| null` and `Published<…>` are all deliberately ignored — the *document's* value
 * decides how to descend, and the type is asked only which shape the leaves are. That is what keeps
 * `Published<Auth>` (C-408) checkable as an `Auth` without this file having to model the wrapper.
 */
function target(declared, known, owner, name) {
  const trimmed = declared.trim()
  if (trimmed.startsWith('{')) {
    return { label: `${owner}.${name}`, fields: fieldsOf(balanced(trimmed, 0).split(';').join('\n')) }
  }
  const named = [...new Set(trimmed.match(/[A-Za-z_$][\w$]*/g) ?? [])].filter((word) =>
    known.has(word)
  )
  assert.ok(
    named.length <= 1,
    `\`${owner}.${name}\` is declared as ${named.join(' | ')} — this check cannot tell which of them a value is, so it would stop covering all of them`
  )
  return named.length ? { label: named[0], fields: known.get(named[0]) } : null
}

/**
 * Every disagreement between the document and the declarations, as sentences.
 *
 * Both directions, because `catalog-json.md` guarantee 1 makes the document *total* — every key
 * always present, an absent value written as `null` or `[]`. A key it publishes and the interface
 * lacks is the drift this story is about; a non-optional field the interface declares and the
 * document never carries is the same defect read from the other end, and under a total document it
 * is just as wrong. A field written `?` is exempt from that second direction and only from that one:
 * `?` is the declaration that the key may be absent, so honouring it is not a hole.
 *
 * `Published<T>` is **not** an exemption. It says a *thinner source* may omit the field; this
 * document is the total one, and holding it to publishing every one of them is exactly the check
 * that would notice the emitter quietly dropping a key that components now branch on.
 */
function disagreements(document, known, seen) {
  // Keyed by the field it is about, so one missing key on one entity is one sentence rather than
  // one per place the document carries that entity — which for an operation is every operation in
  // the catalogue. The first path found travels with it as the example, because an unreadable
  // failure is a failure nobody acts on.
  const found = new Map()
  const report = (field, sentence) => {
    if (!found.has(field)) found.set(field, sentence)
  }

  const walk = (value, shape, where) => {
    if (Array.isArray(value)) {
      value.forEach((element, index) => walk(element, shape, `${where}[${index}]`))
      return
    }
    if (value === null || typeof value !== 'object') return

    seen.set(shape.label, (seen.get(shape.label) ?? 0) + 1)

    for (const key of Object.keys(value)) {
      if (!shape.fields.has(key)) {
        report(
          `+${shape.label}.${key}`,
          `the catalogue publishes \`${key}\` on \`${shape.label}\` (at ${where}) and \`data/catalog.mts\` declares no such field`
        )
      }
    }
    for (const [name, field] of shape.fields) {
      if (!field.optional && !(name in value)) {
        report(
          `-${shape.label}.${name}`,
          `\`data/catalog.mts\` declares \`${shape.label}.${name}\` and the catalogue publishes no such key (at ${where})`
        )
      }
    }

    for (const [key, nested] of Object.entries(value)) {
      const field = shape.fields.get(key)
      if (!field) continue
      const next = target(field.type, known, shape.label, key)
      if (next) walk(nested, next, `${where}.${key}`)
    }
  }

  assert.ok(known.has(ROOT), `\`${ROOT}\` is declared nowhere in data/catalog.mts`)
  walk(document, { label: ROOT, fields: known.get(ROOT) }, ROOT.toLowerCase())
  return [...found.values()]
}

test('the site declares exactly the keys the generated catalogue publishes', () => {
  const known = interfaces(readFileSync(typesPath, 'utf-8'))
  assert.ok(known.size > 0, 'no interface was read out of data/catalog.mts; this would pass vacuously')

  const seen = new Map()
  const found = disagreements(catalog(), known, seen)

  assert.deepEqual(
    found,
    [],
    `the site's types and the document it describes have drifted:\n  - ${found.join('\n  - ')}`
  )

  // And the check reached each entity the story named, so a green run means they were compared
  // rather than absent. A catalogue carrying none of one of these is a real change and says so here.
  for (const name of MUST_BE_EXERCISED) {
    assert.ok(
      (seen.get(name) ?? 0) > 0,
      `the catalogue carries no \`${name}\`, so its declaration was checked against nothing`
    )
  }
})

// The mechanism, pinned in the small rather than inferred from a tree that currently agrees. Above
// asserts the two files match today; these assert that the check would *notice* if they stopped —
// which is the whole claim, and the one that quietly stops being true when a parser is refactored.
test('a key the document publishes and the types omit is named, in both directions', () => {
  const known = interfaces(readFileSync(typesPath, 'utf-8'))
  const shaped = (fields) => `export interface ${ROOT} {\n${fields}\n}\n`

  // The drift this story exists for: the document grew a key and the declaration did not.
  const omitted = interfaces(shaped('  schema_version: number'))
  assert.deepEqual(disagreements({ schema_version: 2, generator: 'x' }, omitted, new Map()), [
    'the catalogue publishes `generator` on `Catalog` (at catalog) and `data/catalog.mts` declares no such field',
  ])

  // The other direction, which a total document makes just as wrong.
  const invented = interfaces(shaped('  schema_version: number\n  absent: string'))
  assert.deepEqual(disagreements({ schema_version: 2 }, invented, new Map()), [
    '`data/catalog.mts` declares `Catalog.absent` and the catalogue publishes no such key (at catalog)',
  ])

  // `?` is the declaration that a key may be absent, and it is honoured in that direction only.
  const optional = interfaces(shaped('  schema_version: number\n  absent?: string'))
  assert.deepEqual(disagreements({ schema_version: 2 }, optional, new Map()), [])

  // Nested entities are reached through the type graph, not through a table of paths, and a name
  // published deep in the document is reported where it was found.
  const nested = interfaces(
    `${shaped('  providers: Provider[]')}export interface Provider {\n  id: string\n}\n`
  )
  assert.deepEqual(disagreements({ providers: [{ id: 'a' }, { id: 'b', extra: 1 }] }, nested, new Map()), [
    'the catalogue publishes `extra` on `Provider` (at catalog.providers[1]) and `data/catalog.mts` declares no such field',
  ])

  // And the real file parses as the real file: prose, JSDoc `{@link}` braces, inheritance and an
  // inline object type are all things the sources actually contain.
  assert.ok(known.get('Hmac').has('timestamp_format'), 'the parser lost a field to the prose above it')
  assert.equal(known.get('ToolSpec').get('group').optional, true, 'the parser lost the one optional')
  assert.ok(known.get('CoreOperation').has('$schema'), 'the parser did not flatten `extends`')
  assert.equal(known.get('Credential').get('scheme').type.startsWith('{'), true)
})
