// The explorer's contract with the generated catalogue (C-44).
//
// Run against a *built* site: `npm run build && npm test`. Everything asserted here is read out of
// `public/catalog.json` and out of the rendered HTML in `.vitepress/dist` — the test never names a
// provider, an operation or an issue code of its own, because that is precisely the failure it
// exists to prevent. Add a fourth provider and this suite covers it with no edit.
//
// Three of these are not staleness checks and are the reason the story exists:
//
//   - **An operation that owns a defect says so wherever it appears** — in the list and on its own
//     page — while an operation that merely inherits a provider- or catalogue-wide condition does
//     not get dressed up as broken. `works` is `false` for all 25 today; presenting that as "0 of 25
//     working" would be accurate and useless.
//   - **Every operation is deep-linkable** at a stable URL, so the site is referenceable from an
//     issue or a chat.
//   - **The content survives without JavaScript.** The pages are pre-rendered, so the assertions
//     read the operation's Flux out of static HTML rather than out of a hydrated app.
//
// Node's built-in test runner, deliberately: the site has exactly one dependency and this adds none.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// The explorer's pure selectors, as a namespace: a selector that does not exist yet then fails the
// test that asks for it, by name, instead of failing this file's import and taking the suite with it.
import * as selectors from '../data/catalog.mts'

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const repoRoot = path.resolve(webRoot, '..')
const catalogPath = path.join(webRoot, 'public', 'catalog.json')
const distDir = path.join(webRoot, '.vitepress', 'dist')

/** The generated catalogue the site ships. */
function catalog() {
  assert.ok(
    existsSync(catalogPath),
    `web/public/catalog.json is missing — the site has no catalogue to read. Run \`cargo run -p connector-cli -- build\``
  )
  return JSON.parse(readFileSync(catalogPath, 'utf-8'))
}

/** Every operation, flattened across providers. */
function operations(document) {
  return document.providers.flatMap((provider) => provider.operations)
}

/** Every Flux-owned entry, kept separate from vendor connector operations. */
function coreEntries(document) {
  assert.ok(document.core, 'the generated catalogue has no Flux core section')
  return [...document.core.operations, ...document.core.nodes, ...document.core.capabilities]
}

/** The issues an operation owns itself, as opposed to the ones it inherits. */
function ownIssues(operation) {
  return operation.status.issues.filter((issue) => issue.scope === 'operation')
}

/**
 * The reserved service name — the one name in this file that is not read out of the catalogue,
 * because it is not catalogue data.
 *
 * It is vocabulary from the address grammar: an operation naming no service belongs to it and it
 * is elided from published addresses. A sole implicit default stays invisible; an explicit legacy
 * default beside named siblings is a real surface whose machine value remains this token.
 */
const RESERVED_SERVICE = 'default'

/** Every filterable/rendered service for a multi-surface provider, and none for one surface. */
function visibleServices(provider) {
  return provider.services.length === 1 && provider.services[0].name === RESERVED_SERVICE
    ? []
    : provider.services
}

/** One built page, as HTML. */
function page(...segments) {
  const file = path.join(distDir, ...segments)
  assert.ok(
    existsSync(file),
    `${path.relative(webRoot, file)} was not built — run \`npm run build\` before \`npm test\``
  )
  return readFileSync(file, 'utf-8')
}

/** The visible text of a page: tags removed, entities resolved, whitespace inside <pre> intact. */
function text(html) {
  return html
    .replace(/<script[\s\S]*?<\/script>/g, ' ')
    .replace(/<style[\s\S]*?<\/style>/g, ' ')
    .replace(/<[^>]+>/g, '')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, '&')
}

/** Every `data-defect` value the page carries for one operation. */
function defectMarkers(html, id) {
  const marker = new RegExp(
    `data-operation="${id}"[^>]*data-defect="(own|none)"|data-defect="(own|none)"[^>]*data-operation="${id}"`,
    'g'
  )
  return [...html.matchAll(marker)].map((match) => match[1] ?? match[2])
}

/** The classes on a built page's doc-layout container, or `null` if it is not a doc page. */
function docLayout(html) {
  const match = html.match(/class="(VPDoc(?: [\w-]+)*)"/)
  return match ? match[1].split(' ') : null
}

/** The theme rule that caps the doc layout's content column, read out of the built stylesheet. */
function contentColumnCap() {
  const assets = path.join(distDir, 'assets')
  assert.ok(existsSync(assets), 'the site was not built — run `npm run build` before `npm test`')
  const css = readdirSync(assets)
    .filter((entry) => entry.endsWith('.css'))
    .map((entry) => readFileSync(path.join(assets, entry), 'utf-8'))
    .join('\n')
  return css.match(/\.VPDoc\.has-aside\s+\.content-container[^{]*\{[^}]*max-width:\s*([^;}]+)/)
}

/** Every stylesheet the built site emits, concatenated. */
function stylesheet() {
  const assets = path.join(distDir, 'assets')
  assert.ok(existsSync(assets), 'the site was not built — run `npm run build` before `npm test`')
  return readdirSync(assets)
    .filter((entry) => entry.endsWith('.css'))
    .map((entry) => readFileSync(path.join(assets, entry), 'utf-8'))
    .join('\n')
}

/** Every file under the explorer's own sources — where hand-maintained data would have to live. */
function explorerSources() {
  const config = path.join(webRoot, '.vitepress')
  const roots = [
    path.join(webRoot, 'data'),
    path.join(webRoot, 'operations'),
    path.join(config, 'theme'),
  ]
  const files = [path.join(webRoot, 'explorer.md')]
  const walk = (dir) => {
    if (!existsSync(dir)) return
    for (const entry of readdirSync(dir)) {
      const full = path.join(dir, entry)
      if (statSync(full).isDirectory()) walk(full)
      else files.push(full)
    }
  }
  roots.forEach(walk)
  // C-205. Markdown under `.vitepress/` is not content. VitePress routes pages from the site root
  // and `.vitepress/` is its config and theme directory, so `theme/components/README.md` is built
  // into no page — the 328 pages in `dist` include neither README the repo carries. It is notes for
  // whoever edits the components, which is the same category as a comment and is read the same way:
  // not at all. Page Markdown — `explorer.md`, `operations/[operation].md` — is still a source.
  return files
    .filter(existsSync)
    .filter((file) => !(path.extname(file) === '.md' && file.startsWith(config + path.sep)))
}

/** Every Vue component the explorer is built from, as absolute paths in directory order. */
function componentSources() {
  const dir = path.join(webRoot, '.vitepress', 'theme', 'components')
  return readdirSync(dir)
    .filter((entry) => entry.endsWith('.vue'))
    .map((entry) => path.join(dir, entry))
}

/**
 * Every module specifier a source imports from.
 *
 * Anchored at the start of a line, so a specifier named in a comment — and these files carry a lot
 * of comment — is not mistaken for an import.
 */
function importedModules(source) {
  return [...source.matchAll(/^\s*import\b[\s\S]*?\bfrom\s*'([^']+)'/gm)].map((match) => match[1])
}

/**
 * Script comments removed, string literals kept.
 *
 * String-aware on purpose rather than a `//.*$` sweep: a hard-coded `https://api.postmark.com` is
 * exactly what the guard below exists to catch, and a line sweep would cut it at its own `//` and
 * hide it. Comments are replaced by a space so removing one cannot join its neighbours into a token
 * that was never written.
 *
 * A quote that opens no string — an apostrophe in a stray position — makes the scanner read the
 * rest of the file as string content, so it keeps text rather than dropping it. Every ambiguity
 * here fails towards matching more, which is the safe direction for a guard.
 */
function withoutScriptComments(source) {
  let kept = ''
  for (let i = 0; i < source.length; ) {
    const pair = source.slice(i, i + 2)
    if (pair === '//') {
      const end = source.indexOf('\n', i)
      kept += ' '
      i = end === -1 ? source.length : end
    } else if (pair === '/*') {
      const end = source.indexOf('*/', i + 2)
      kept += ' '
      i = end === -1 ? source.length : end + 2
    } else if (source[i] === "'" || source[i] === '"' || source[i] === '`') {
      const quote = source[i]
      const start = i++
      while (i < source.length && source[i] !== quote) i += source[i] === '\\' ? 2 : 1
      kept += source.slice(start, ++i)
    } else {
      kept += source[i++]
    }
  }
  return kept
}

/** Style comments removed. CSS has only the one form. */
function withoutStyleComments(source) {
  return source.replace(/\/\*[\s\S]*?\*\//g, ' ')
}

/** Markup comments removed. The one form Markdown and a Vue template share. */
function withoutMarkupComments(source) {
  return source.replace(/<!--[\s\S]*?-->/g, ' ')
}

/**
 * What a source contributes to the built site: its code, its markup and its rendered text, with the
 * prose written *about* them removed.
 *
 * C-205. The guard below greps the explorer's sources for catalogue values. Read as raw text it
 * greps their comments too, and **thirteen catalogue service names are ordinary English words** —
 * `account`, `admin`, `calendar`, `default`, `delivery`, `drive`, `files`, `gmail`, `machines`,
 * `mail`, `management`, `models`, `server`. Postmark shipping a service called `server` turned a
 * sentence about the VitePress dev server into a gate failure. An allowlist would be that bug filed
 * once per connector, and "do not use these words in a comment" is a rule no author can follow.
 *
 * So the narrowing is on what hand-maintained data *is*, not on which words are exempt: data
 * hand-written into the site is a value the site can **render** — a literal in code, an attribute
 * or text in markup, the body of a page. A comment renders nothing, so a comment is never
 * hand-maintained catalogue data whatever words it happens to contain. Everything that is not a
 * comment is still matched as raw text, exactly as it was, so nothing that reaches a reader has
 * become invisible to the guard.
 *
 * A Vue file is three languages, so its `<script>` and `<style>` blocks are read as such and its
 * template as markup.
 */
function renderedSource(file, source) {
  const script = ['.mts', '.cts', '.ts', '.mjs', '.cjs', '.js']
  const extension = path.extname(file)
  if (script.includes(extension)) return withoutScriptComments(source)
  if (extension === '.css') return withoutStyleComments(source)
  if (extension === '.md') return withoutMarkupComments(source)
  if (extension !== '.vue') return source

  const blocks = /(<(script|style)\b[^>]*>)([\s\S]*?)(<\/\2>)/g
  let kept = ''
  let read = 0
  for (const block of source.matchAll(blocks)) {
    const [whole, open, language, body, close] = block
    kept += withoutMarkupComments(source.slice(read, block.index))
    kept += open + (language === 'script' ? withoutScriptComments(body) : withoutStyleComments(body)) + close
    read = block.index + whole.length
  }
  return kept + withoutMarkupComments(source.slice(read))
}

/**
 * Every value in the generated catalogue that the site must render rather than restate.
 *
 * Read out of the document, so a new provider is covered with no edit here — which is the property
 * the guard that uses this exists to defend.
 */
function catalogueValues(document) {
  const values = new Set()
  for (const provider of document.providers) {
    values.add(provider.id)
    values.add(provider.vendor)
    values.add(provider.base_url)
    provider.hosts.forEach((host) => values.add(host))
    provider.auth.credentials.forEach((credential) => values.add(credential.name))
    // A service name and a published address are catalogue data like any other. The reserved name
    // is the one exception, and only because it is not data: it is grammar the site has to know
    // about in order to render nothing for it.
    for (const service of provider.services) {
      if (service.name !== RESERVED_SERVICE) values.add(service.name)
      if (service.gid) values.add(service.gid)
    }
    for (const operation of provider.operations) {
      values.add(operation.id)
      operation.status.issues.forEach((issue) => values.add(issue.code))
    }
    // C-83. A member's rendered address is catalogue data like an operation id, and the inbound
    // components render one, so hard-coding one is the same failure.
    //
    // The bare *names* are deliberately not here, and that is not an oversight: an event keeps its
    // vendor spelling, and vendors spell them as ordinary English words — one shipped connector
    // declares an event called `message`. C-205 narrowed the *matching* rather than the set, so a
    // name in a comment is no longer a find; a name rendered into a component still would be.
    for (const member of [...provider.events, ...provider.channels]) {
      if (member.oip) values.add(member.oip)
    }
  }
  return values
}

/**
 * Whether a text names a catalogue value, as opposed to merely containing its letters.
 *
 * C-205, the second narrowing. `data/catalog.mts` declares the shape of the document it loads, and
 * one of the fields is `delivery_id` — which contains the letters of the service `delivery`. A
 * field name is structure, not data, and `delivery_id` is no more the service `delivery` than
 * `gmail` is the service `mail` or `drives` is the service `drive`; the story records both of those
 * as the same misread. So a value counts when it stands as a word, not as a fragment of a longer
 * one.
 *
 * The trailing boundary treats a capital as the end of a word, because an identifier does: a
 * hard-coded `zendeskTicket` still names `zendesk` and is still caught. The leading boundary is a
 * plain word character, which is why `gmail` does not report `mail`.
 */
function namesValue(text, value) {
  for (let at = text.indexOf(value); at !== -1; at = text.indexOf(value, at + 1)) {
    const before = at === 0 ? '' : text[at - 1]
    const after = text[at + value.length] ?? ''
    if (!/[A-Za-z0-9_]/.test(before) && !/[a-z0-9_]/.test(after)) return true
  }
  return false
}

/**
 * Every catalogue value a source hand-writes into the site, as `{ file, value }`.
 *
 * Takes the sources as `{ file, source }` rather than reading them, so a test can ask the question
 * of a source that is not on disk — which is how the two halves of this guard are proved: that
 * prose is tolerated, and that hand-written data is still caught.
 */
function handMaintainedData(sources, values) {
  const found = []
  for (const { file, source } of sources) {
    const rendered = renderedSource(file, source)
    for (const value of values) {
      if (namesValue(rendered, value)) found.push({ file: path.relative(webRoot, file), value })
    }
  }
  return found
}

test('the site ships the generated catalogue at the path VitePress serves', () => {
  const document = catalog()

  assert.equal(document.schema_version, 3)
  assert.ok(document.providers.length > 0, 'the catalogue names no providers')
  assert.ok(operations(document).length > 0, 'the catalogue names no operations')

  // The document moved into the site it is written for; a second copy at the old path would be a
  // second source of truth.
  assert.ok(
    !existsSync(path.join(repoRoot, 'site', 'catalog.json')),
    'site/catalog.json still exists alongside web/public/catalog.json — one of them is stale by construction'
  )
})

test('the public catalogue and pages do not expose internal project documents', () => {
  const document = catalog()

  assert.ok(!('documentation' in document), 'catalog.json publishes its internal design document')
  for (const operation of operations(document)) {
    for (const issue of operation.status.issues) {
      assert.ok(!('story' in issue), `issue ${issue.code} publishes its internal story`)
      for (const internal of ['http.request', '$secret', 'auth seam', 'owning story']) {
        assert.ok(
          !issue.summary.includes(internal),
          `issue ${issue.code} exposes implementation detail ${internal}`
        )
      }
    }
  }

  const publicHtml = [page('index.html'), page('explorer.html')]
  for (const operation of operations(document)) {
    publicHtml.push(page('operations', `${operation.id}.html`))
  }
  const source = publicHtml.join('\n')
  for (const internal of ['docs/designs/', 'docs/roadmap.md', 'docs/stories/', 'AGENTS.md']) {
    assert.ok(!source.includes(internal), `the public site exposes internal path ${internal}`)
  }

  for (const operation of operations(document)) {
    const detail = page('operations', `${operation.id}.html`)
    assert.doesNotMatch(detail, /Previous page|Next page/, `${operation.id} has an unrelated pager`)
  }
})

test('the published logo and mark match the canonical brand assets', () => {
  for (const name of ['icon.svg', 'mark.svg']) {
    const canonical = readFileSync(path.join(repoRoot, 'assets', 'brand', name), 'utf-8')
    const published = readFileSync(path.join(webRoot, 'public', 'brand', name), 'utf-8')
    assert.equal(published, canonical, `web/public/brand/${name} drifted from assets/brand/${name}`)
  }
})

test('every operation has its own deep-linkable page', () => {
  for (const operation of operations(catalog())) {
    page('operations', `${operation.id}.html`)
  }
})

test('the Flux core catalogue is complete, versioned, and does not invent a noop', () => {
  const document = catalog()
  const core = document.core
  assert.ok(core, 'catalog.json has no Flux-owned core catalogue')
  assert.equal(core.schema_version, 1)
  assert.ok(core.operations.length > 0, 'the core catalogue names no operations')
  assert.ok(core.nodes.length > 0, 'the core catalogue names no language nodes')
  assert.ok(core.capabilities.length > 0, 'the core catalogue names no network capabilities')
  assert.ok(!coreEntries(document).some((entry) => entry.name === 'noop'))

  const ids = coreEntries(document).map((entry) => entry.$id)
  assert.equal(new Set(ids).size, ids.length, 'two core entries publish the same canonical id')

  for (const entry of coreEntries(document)) {
    assert.ok(entry.$id.startsWith('https://flux.codewandler.org/v1/'))
    const relative = entry.$id.slice('https://flux.codewandler.org/'.length)
    const published = path.join(webRoot, 'public', relative)
    assert.ok(existsSync(published), `${entry.$id} has no published JSON document`)
    assert.deepEqual(JSON.parse(readFileSync(published, 'utf-8')), entry)
  }

  for (const schema of Object.values(core.schemas)) {
    const relative = schema.$id.slice('https://flux.codewandler.org/'.length)
    assert.deepEqual(
      JSON.parse(readFileSync(path.join(webRoot, 'public', relative), 'utf-8')),
      schema
    )
  }
})

test('every Flux core entry has a static detail page with its contract and canonical spec', () => {
  const document = catalog()
  for (const entry of coreEntries(document)) {
    const kind = entry.kind === 'capability' ? 'capabilities' : `${entry.kind}s`
    const body = text(page('core', kind, `${entry.name}.html`))
    assert.ok(body.includes(entry.description), `${entry.kind} ${entry.name} loses its description`)
    assert.ok(body.includes(entry.$id), `${entry.kind} ${entry.name} loses its canonical JSON id`)

    if (entry.kind === 'operation') {
      assert.ok(body.includes(entry.tool_spec.risk), `${entry.name} loses its risk`)
      assert.ok(body.includes(entry.tool_spec.idempotency), `${entry.name} loses its idempotency`)
      assert.ok(body.includes(JSON.stringify(entry.tool_spec.input_schema, null, 2)))
    } else if (entry.kind === 'node') {
      assert.ok(body.includes(entry.schema_ref), `${entry.name} loses its AST schema anchor`)
    } else {
      assert.ok(body.includes(entry.callable ? 'callable' : 'not callable'))
      for (const id of entry.operation_ids) assert.ok(body.includes(id))
    }
  }
})

test('planned network capabilities are clearly non-callable everywhere they appear', () => {
  const document = catalog()
  const planned = document.core.capabilities.filter((entry) => entry.availability === 'planned')
  assert.ok(planned.length > 0, 'the planned capability state is not exercised')

  const explorer = page('explorer.html')
  for (const entry of planned) {
    assert.equal(entry.callable, false)
    assert.deepEqual(entry.operation_ids, [])
    assert.match(
      explorer,
      new RegExp(`data-core-name="${entry.name}"[^>]*data-availability="planned"[^>]*data-callable="false"`)
    )
    assert.ok(text(page('core', 'capabilities', `${entry.name}.html`)).includes('not callable'))
  }
})

// A previous version of this test asserted `base === '/'` because `public/CNAME` names a custom
// domain. That is exactly the reasoning that shipped an unstyled site: a committed CNAME is a
// *request* for a custom domain, not evidence one is serving. GitHub never accepted it — the Pages
// API reports `"cname": null` and still serves the project-pages URL — so every asset 404'd.
//
// So this asserts the one thing a file can actually prove: that the base the site is built with and
// the base its own emitted HTML uses are the same string, and that it is the project-pages prefix
// the deployment is known to serve from. Whoever moves the site to the custom domain flips both
// halves here, and should confirm the move first with
// `gh api repos/codewandler/flux-connectors/pages --jq .cname`.
test('the site is built for the path GitHub Pages actually serves it from', () => {
  const config = readFileSync(path.join(webRoot, '.vitepress', 'config.mts'), 'utf-8')
  assert.match(config, /const base = '\/flux-connectors\/'/)

  // The built HTML is the artifact that gets deployed, so it is what must carry the prefix.
  const home = page('index.html')
  const assets = [...home.matchAll(/(?:href|src)="(\/[^"]*\/assets\/[^"]+)"/g)].map((m) => m[1])
  assert.ok(assets.length > 0, 'the built home page links no bundled assets; this would pass vacuously')
  for (const url of assets) {
    assert.ok(
      url.startsWith('/flux-connectors/assets/'),
      `\`${url}\` is not under the deployed base, so it 404s and the page renders unstyled`
    )
  }
})

test('operation-owned defects are rendered exactly when the catalogue publishes one', () => {
  const document = catalog()
  const explorer = page('explorer.html')
  const owners = operations(document).filter((operation) => ownIssues(operation).length > 0)

  if (owners.length === 0) {
    const falseOwners = operations(document).filter((operation) =>
      defectMarkers(explorer, operation.id).includes('own')
    )
    assert.deepEqual(
      falseOwners,
      [],
      'the explorer marks an operation as owning a defect even though the catalogue publishes none'
    )
    return
  }

  for (const operation of owners) {
    assert.deepEqual(
      [...new Set(defectMarkers(explorer, operation.id))],
      ['own'],
      `the operation list does not mark \`${operation.id}\` as owning a defect`
    )

    const detail = page('operations', `${operation.id}.html`)
    assert.deepEqual(
      [...new Set(defectMarkers(detail, operation.id))],
      ['own'],
      `the page for \`${operation.id}\` does not mark it as owning a defect`
    )

    // The reason, not just the badge.
    const body = text(detail)
    for (const issue of ownIssues(operation)) {
      assert.ok(
        body.includes(issue.summary),
        `the page for \`${operation.id}\` does not explain its defect`
      )
      for (const parameter of issue.params) {
        assert.ok(
          body.includes(parameter),
          `the page for \`${operation.id}\` does not name the implicated parameter \`${parameter}\``
        )
      }
    }
  }
})

test('an operation that only inherits a wider condition is not presented as broken', () => {
  const document = catalog()
  const explorer = page('explorer.html')
  const inheritors = operations(document).filter((operation) => ownIssues(operation).length === 0)

  assert.ok(inheritors.length > 0, 'every operation owns a defect; this assertion would pass vacuously')

  for (const operation of inheritors) {
    assert.deepEqual(
      [...new Set(defectMarkers(explorer, operation.id))],
      ['none'],
      `the operation list presents \`${operation.id}\` as owning a defect it does not own`
    )
    assert.deepEqual(
      [...new Set(defectMarkers(page('operations', `${operation.id}.html`), operation.id))],
      ['none'],
      `the page for \`${operation.id}\` presents it as owning a defect it does not own`
    )
  }

  // `works` is false for all 25 operations today, correctly — no provider can make a live call. A
  // headline counting that as "0 of 25 working" is accurate and useless.
  assert.doesNotMatch(
    text(explorer),
    /\b0 of \d+\b/,
    'the explorer counts operations by `works`, which today reads as "0 of N" and tells a visitor nothing'
  )
})

test('conditions wider than one operation are presented as banners, counted from the data', () => {
  const document = catalog()
  const explorer = page('explorer.html')

  const wider = (scope) =>
    new Map(
      operations(document)
        .flatMap((operation) => operation.status.issues)
        .filter((issue) => issue.scope === scope)
        .map((issue) => [issue.summary, issue])
    )

  const catalogWide = wider('catalog')
  assert.ok(catalogWide.size > 0, 'the catalogue declares no catalogue-wide condition')
  assert.match(explorer, /data-banner="catalog"/, 'no catalogue-wide banner is rendered')
  for (const issue of catalogWide.values()) {
    assert.ok(text(explorer).includes(issue.summary), 'a catalogue-wide condition is not stated')
  }

  for (const provider of document.providers) {
    const scoped = provider.operations
      .flatMap((operation) => operation.status.issues)
      .filter((issue) => issue.scope === 'provider')
    if (scoped.length === 0) continue
    assert.match(
      explorer,
      new RegExp(`data-banner="provider"[^>]*data-provider="${provider.id}"`),
      `\`${provider.id}\` has a provider-wide condition and no banner for it`
    )
  }

  // The headline count is the number of operations that own a defect, derived here from the data.
  const owned = operations(document).filter((operation) => ownIssues(operation).length > 0).length
  assert.match(
    explorer,
    new RegExp(`data-defect-count="${owned}"`),
    `the explorer does not report ${owned} operations owning a defect`
  )
})

test('an operation page carries its signature, parameters, Flux, credentials and hosts without JavaScript', () => {
  const document = catalog()

  for (const provider of document.providers) {
    for (const operation of provider.operations) {
      const body = text(page('operations', `${operation.id}.html`))

      assert.ok(body.includes(operation.method), `\`${operation.id}\` does not show its method`)
      assert.ok(body.includes(operation.path), `\`${operation.id}\` does not show its path`)
      assert.ok(
        body.includes(operation.flux.trimEnd()),
        `\`${operation.id}\` does not carry its generated Flux verbatim in static HTML`
      )

      for (const parameter of operation.parameters) {
        assert.ok(
          body.includes(parameter.name),
          `\`${operation.id}\` does not show its parameter \`${parameter.name}\``
        )
        // The vendor's JSON Schema, verbatim — constraint keywords included, not a stringly-typed
        // shadow of it.
        assert.ok(
          body.includes(JSON.stringify(parameter.schema, null, 2)),
          `parameter \`${parameter.name}\` of \`${operation.id}\` loses its JSON Schema`
        )
      }

      if (operation.body_schema) {
        assert.ok(
          body.includes(JSON.stringify(operation.body_schema, null, 2)),
          `\`${operation.id}\` takes a schema-shaped body and does not show it`
        )
      }

      for (const alternative of operation.credentials) {
        for (const credential of alternative) {
          assert.ok(
            body.includes(credential),
            `\`${operation.id}\` does not name the credential \`${credential}\``
          )
        }
      }

      for (const host of operation.hosts) {
        assert.ok(body.includes(host), `\`${operation.id}\` does not name the host \`${host}\``)
      }
    }
  }
})

test('the explorer is outside the content column that constrains the prose pages', () => {
  // C-100. The doc layout caps its content column at a fixed width, and that cap is not a variable
  // a page can raise — it is a rule keyed on `has-aside`. So the page that must be wide is the page
  // that must not carry an aside, and the assertion is on that class rather than on a screenshot.
  //
  // Widening *every* page would be the regression: the doc column is right for paragraphs, so each
  // prose page is checked to be still inside it.
  const cap = contentColumnCap()
  assert.ok(
    cap,
    'the theme no longer caps `.VPDoc.has-aside .content-container` — the reasoning behind C-100 has to be re-derived against the current VitePress'
  )

  const explorer = docLayout(page('explorer.html'))
  assert.ok(explorer, 'the explorer is not rendered by the doc layout at all')
  assert.ok(
    !explorer.includes('has-aside'),
    `the explorer still renders inside the ${cap[1].trim()} content column (VPDoc classes: ${explorer.join(' ')}) — 16 provider cards and 88 operations do not fit there`
  )
  assert.doesNotMatch(
    page('explorer.html'),
    /VPDocAside/,
    'the explorer still renders the outline aside, which reimposes the capped content column'
  )

  // Dropping the outline is only acceptable because the section headings remain link targets; they
  // are linked from elsewhere.
  for (const anchor of ['core', 'providers', 'operations']) {
    assert.match(
      page('explorer.html'),
      new RegExp(`id="${anchor}"`),
      `the explorer no longer offers the \`#${anchor}\` anchor, which is linked from elsewhere`
    )
  }

  for (const operation of operations(catalog())) {
    const detail = docLayout(page('operations', `${operation.id}.html`))
    assert.ok(
      detail?.includes('has-aside'),
      `the page for \`${operation.id}\` left the ${cap[1].trim()} content column — the doc layout is right for prose and only the explorer was meant to leave it`
    )
  }
})

test('the explorer lists every provider and every operation without JavaScript', () => {
  const document = catalog()
  const body = text(page('explorer.html'))

  for (const provider of document.providers) {
    assert.ok(body.includes(provider.vendor), `the provider list omits \`${provider.vendor}\``)
    assert.ok(
      body.includes(String(provider.operation_count)),
      `the provider list omits the operation count for \`${provider.id}\``
    )
    for (const operation of provider.operations) {
      assert.ok(body.includes(operation.id), `the operation list omits \`${operation.id}\``)
    }
  }
})

test('provider cards render every declared configuration field without JavaScript', () => {
  const document = catalog()
  const html = page('explorer.html')
  const body = text(html)
  let rendered = 0

  for (const provider of document.providers) {
    assert.ok(Array.isArray(provider.config), `\`${provider.id}\` publishes no configuration array`)
    for (const field of provider.config) {
      rendered += 1
      assert.match(
        html,
        new RegExp(
          `data-config-of="${provider.id}"[^>]*data-config-field="${field.name}"|` +
            `data-config-field="${field.name}"[^>]*data-config-of="${provider.id}"`
        ),
        `the card for \`${provider.id}\` does not render configuration field \`${field.name}\``
      )
      assert.ok(body.includes(field.label), `the form omits the label for \`${provider.id}.${field.name}\``)
      assert.ok(body.includes(field.help), `the form omits the help for \`${provider.id}.${field.name}\``)
    }
  }

  assert.ok(rendered > 0, 'no shipped connector declares configuration, so the rendering gate is vacuous')
})

test('provider cards render every published Test connection operation without JavaScript', () => {
  const document = catalog()
  const html = page('explorer.html')
  let rendered = 0

  for (const provider of document.providers) {
    if (!provider.verify) continue
    rendered += 1
    assert.ok(
      html.includes(`data-verify-of="${provider.id}"`),
      `the card for \`${provider.id}\` does not render its Test connection operation \`${provider.verify}\``
    )
  }

  assert.ok(rendered > 0, 'no shipped connector publishes `verify`, so the rendering gate is vacuous')
})

test('provider cards render Test connection even when the connector declares no configuration fields', () => {
  const card = markup(component('ProviderCard.vue'))

  assert.match(
    card,
    /<\/details>\s*<p\s+v-if="provider\.verify\s*&&\s*!provider\.config\.length"[\s\S]*?class="config-verify"[\s\S]*?>\s*Test connection with/,
    'the Test connection row has no config-independent fallback; a connector with `verify` and an empty config array renders nothing'
  )
})

test('operator-approved configuration is typed and rendered as an activation policy', () => {
  const catalogue = readFileSync(path.join(webRoot, 'data', 'catalog.mts'), 'utf-8')
  const card = component('ProviderCard.vue')

  assert.match(
    catalogue,
    /approval\?: 'operator'/,
    'the public catalogue type does not carry the operator approval policy'
  )
  assert.match(
    card,
    /field\.approval === 'operator'/,
    'the provider card does not branch on the published approval policy'
  )
  assert.match(
    markup(card),
    /operator approval required/,
    'the provider card does not explain that a proposed value is not active'
  )
})

// ---------------------------------------------------------------------------------------------
// C-83 — the inbound surface.
//
// The site described half of what a connector does: an operation is flux calling the vendor and had
// a page, a row and a filter, while an event is the vendor calling flux and had nothing, because
// until C-83 it reached no artifact for a site to read.
//
// Both checks below carry a vacuity guard, for the reason every other check here does: a connector
// with no inbound surface contributes nothing, so without one an edit that dropped every binding
// would make these pass rather than fail.
// ---------------------------------------------------------------------------------------------

test('every declared channel binding is rendered on its connector, with what it carries', () => {
  const document = catalog()
  const html = page('explorer.html')
  const body = text(html)

  let rendered = 0
  for (const provider of document.providers) {
    assert.ok(
      Array.isArray(provider.channels) && Array.isArray(provider.events),
      `\`${provider.id}\` publishes no inbound arrays — an absent value is \`[]\`, never a missing key`
    )

    for (const channel of provider.channels) {
      rendered += 1
      assert.ok(
        html.includes(`data-channel="${channel.name}"`),
        `the card for \`${provider.id}\` does not render its binding \`${channel.name}\``
      )
      assert.ok(
        body.includes(channel.transport),
        `the binding \`${channel.name}\` does not say which transport it rides on`
      )
      assert.ok(
        html.includes(`data-payload-root="${channel.payload_root}"`),
        `the binding \`${channel.name}\` does not state whether it delivers the complete payload`
      )
      if (channel.connect) {
        assert.ok(
          body.includes(channel.connect.path),
          `the binding \`${channel.name}\` does not render its socket path \`${channel.connect.path}\``
        )
        for (const alternative of channel.connect.auth ?? []) {
          for (const credential of alternative.credentials) {
            assert.ok(
              body.includes(credential),
              `the binding \`${channel.name}\` does not name its socket credential \`${credential}\``
            )
          }
        }
      }
      for (const event of channel.events) {
        assert.ok(
          body.includes(event),
          `the binding \`${channel.name}\` does not name the event \`${event}\` it carries`
        )
        const declaration = provider.events.find((candidate) => candidate.name === event)
        if (declaration?.wire_value) {
          assert.ok(
            body.includes(declaration.wire_value),
            `the binding \`${channel.name}\` does not show the wire event \`${declaration.wire_value}\``
          )
        }
      }
      // The reply as the address a consumer copies, falling back to the local id for a connector
      // that publishes no authority.
      if (channel.reply) {
        const address = channel.reply.oip ?? channel.reply.operation
        assert.ok(
          body.includes(address),
          `the binding \`${channel.name}\` does not say what answers it (\`${address}\`)`
        )
      }
    }
  }

  assert.ok(
    rendered > 0,
    'no connector in the catalogue declares a channel binding, so this test asserts nothing'
  )
})

test('a binding that cannot be verified is rendered as unverified, from a value and not an absence', () => {
  const document = catalog()
  const html = page('explorer.html')

  let checked = 0
  for (const provider of document.providers) {
    for (const channel of provider.channels) {
      checked += 1

      // The catalogue's own contract first: a verification block is always present and always
      // carries both keys, so telling a signed surface from an open one never means testing for
      // existence. This is C-82's "silence is never a verification answer" as a property of the
      // document the site is written against.
      assert.ok(
        channel.verification && typeof channel.verification.kind === 'string',
        `the binding \`${channel.name}\` publishes no verification kind`
      )
      assert.equal(
        channel.verification.verified,
        channel.verification.kind !== 'none',
        `the binding \`${channel.name}\` reports \`verified\` out of step with its kind`
      )

      assert.ok(
        html.includes(`data-verified="${channel.verification.verified}"`),
        `the card does not mark \`${channel.name}\` as verified=${channel.verification.verified}`
      )
    }
  }

  assert.ok(checked > 0, 'no binding was checked; this test would pass vacuously')

  // And the selectors agree with the markup, so a component cannot answer this question its own way.
  for (const provider of document.providers) {
    assert.deepEqual(
      selectors.unverifiedChannels(provider).map((channel) => channel.name),
      provider.channels
        .filter((channel) => !channel.verification.verified)
        .map((channel) => channel.name)
    )
  }
})

test('the inbound selectors join a binding to its events and prefer a published address', () => {
  const verified = { kind: 'hmac', verified: true, hmac: null }
  const event = (name, rest = {}) => ({
    name,
    wire_value: null,
    service: RESERVED_SERVICE,
    oip: null,
    description: '',
    default: true,
    group: '',
    when: {},
    schema: null,
    ...rest,
  })
  const channel = (rest = {}) => ({
    name: 'binding',
    service: RESERVED_SERVICE,
    oip: null,
    description: '',
    transport: 'webhook',
    connect: null,
    events: [],
    verification: verified,
    discriminator: null,
    delivery_id: null,
    payload: {},
    payload_root: false,
    reply: null,
    cursor: null,
    interval: null,
    subscription: null,
    setup: null,
    ...rest,
  })

  const provider = {
    id: 'fixture',
    events: [event('one'), event('two'), event('three')],
    channels: [channel({ events: ['two', 'one'] })],
  }

  assert.ok(selectors.hasInboundSurface(provider))
  assert.ok(!selectors.hasInboundSurface({ ...provider, events: [], channels: [] }))

  // The binding's own order, not the connector's declaration order.
  assert.deepEqual(
    selectors.channelEvents(provider, provider.channels[0]).map((carried) => carried.name),
    ['two', 'one']
  )

  // A name with no declaration is dropped rather than rendered as a stub: the loader refuses one, so
  // inventing a placeholder would put a capability on the page the connector does not have.
  assert.deepEqual(
    selectors.channelEvents(provider, channel({ events: ['nothing-declares-this'] })),
    []
  )

  // The address a consumer copies: the oip when there is one, the local id otherwise, and nothing
  // at all for a fire-and-forget binding.
  assert.equal(selectors.replyAddress(channel()), null)
  assert.equal(
    selectors.replyAddress(channel({ reply: { operation: 'op', oip: null, result: null, bind: {} } })),
    'op'
  )
  assert.equal(
    selectors.replyAddress(
      channel({ reply: { operation: 'op', oip: 'com.acme.api:v1#op', result: null, bind: {} } })
    ),
    'com.acme.api:v1#op'
  )

  // An unknown kind shows the raw token rather than a label that would present it as understood.
  assert.equal(selectors.verificationLabel(channel()), 'Signed')
  assert.equal(
    selectors.verificationLabel(channel({ verification: { kind: 'none', verified: false, hmac: null } })),
    'Unverified'
  )
  assert.equal(
    selectors.verificationLabel(
      channel({ verification: { kind: 'a-scheme-this-build-has-not-heard-of', verified: true, hmac: null } })
    ),
    'a-scheme-this-build-has-not-heard-of'
  )
})

test('the service filter is a facet of the catalogue and narrows to the chosen connector', () => {
  const providers = catalog().providers

  const published = [
    ...new Set(providers.flatMap((provider) => visibleServices(provider).map((s) => s.name))),
  ]
  assert.ok(published.length > 0, 'no connector publishes several services; this would pass vacuously')

  // With no connector chosen, every surface of a multi-surface connector is on offer.
  assert.deepEqual(selectors.serviceFacet(providers), published)

  // Choosing a connector narrows the options to that connector's own, in catalogue order.
  for (const provider of providers) {
    assert.deepEqual(
      selectors.serviceFacet(providers, provider.id),
      visibleServices(provider).map((service) => service.name),
      `choosing \`${provider.id}\` does not narrow the service options to its own`
    )
  }

  // The narrowing is worth having only if the catalogue actually varies: at least one connector
  // publishes several services and at least one publishes none of its own.
  const several = providers.filter((provider) => visibleServices(provider).length > 1)
  const single = providers.filter((provider) => visibleServices(provider).length === 0)
  assert.ok(several.length > 0, 'no connector publishes more than one service')
  assert.ok(single.length > 0, 'every connector publishes a named service')

  // A chosen service never empties the list by construction: every operation of a multi-service
  // connector belongs to one of the options that connector offers.
  for (const provider of several) {
    const options = selectors.serviceFacet(providers, provider.id)
    for (const operation of provider.operations) {
      assert.ok(
        options.includes(operation.service),
        `\`${operation.id}\` belongs to a service its connector does not offer as an option`
      )
    }
  }
})

test('Zendesk primary Support is filterable as default while a sole default stays omitted', () => {
  const providers = catalog().providers
  const zendesk = providers.find((provider) => provider.id === 'zendesk')
  assert.ok(zendesk, 'the Zendesk suite fixture is absent')
  assert.ok(zendesk.services.length > 1, 'Zendesk no longer exercises a legacy default beside siblings')

  const primary = zendesk.services.find((service) => service.name === RESERVED_SERVICE)
  assert.ok(primary, 'Zendesk no longer carries its primary Support surface as `default`')
  const support = zendesk.operations.find((operation) => operation.service === RESERVED_SERVICE)
  assert.ok(support, 'Zendesk publishes no operation on its primary Support surface')

  assert.deepEqual(
    selectors.serviceFacet(providers, zendesk.id),
    zendesk.services.map((service) => service.name),
    'Zendesk does not offer every service machine value'
  )
  assert.equal(selectors.serviceLabel(RESERVED_SERVICE), 'Primary')
  assert.equal(selectors.serviceLabel('help-center'), 'help-center')
  assert.equal(selectors.operationService(zendesk, support), RESERVED_SERVICE)
  assert.equal(
    selectors.narrowView(
      { ...selectors.emptyView(), provider: zendesk.id, service: RESERVED_SERVICE },
      providers
    ).service,
    RESERVED_SERVICE,
    'the default machine value is discarded before filtering'
  )
  const primaryView = {
    ...selectors.emptyView(),
    provider: zendesk.id,
    service: RESERVED_SERVICE,
  }
  assert.deepEqual(
    zendesk.operations
      .filter((operation) => selectors.operationMatchesView(zendesk, operation, primaryView))
      .map((operation) => operation.id),
    zendesk.operations
      .filter((operation) => operation.service === RESERVED_SERVICE)
      .map((operation) => operation.id),
    'filtering the raw default value does not return exactly the primary operations'
  )

  const single = providers.find(
    (provider) =>
      provider.services.length === 1 && provider.services[0].name === RESERVED_SERVICE
  )
  assert.ok(single, 'no single-surface default connector exercises omission')
  assert.deepEqual(selectors.serviceFacet(providers, single.id), [])
  assert.equal(selectors.operationService(single, single.operations[0]), null)

  const explorer = page('explorer.html')
  assert.match(explorer, /<option value="default"[^>]*>Primary<\/option>/)

  const zendeskStart = explorer.indexOf(`id="${zendesk.id}"`)
  const zendeskCard = explorer.slice(zendeskStart, explorer.indexOf('</section>', zendeskStart))
  const primaryEntry = zendeskCard.slice(zendeskCard.indexOf('data-service="default"'))
  assert.ok(primaryEntry.startsWith('data-service="default"'), 'the primary card lost its raw value')
  assert.ok(
    text(primaryEntry.slice(0, primaryEntry.indexOf('</li>'))).includes('Primary'),
    'the primary card renders the reserved token instead of its generic label'
  )

  const supportStart = explorer.indexOf(`data-operation="${support.id}"`)
  const supportRow = explorer.slice(supportStart, explorer.indexOf('</li>', supportStart))
  assert.match(supportRow, /data-service="default"/)
  assert.ok(text(supportRow).includes('Primary'))

  const singleStart = explorer.indexOf(`id="${single.id}"`)
  const singleCard = explorer.slice(singleStart, explorer.indexOf('</section>', singleStart))
  assert.doesNotMatch(singleCard, /data-service-of=/)
  const singleOperation = single.operations[0]
  const rowStart = explorer.indexOf(`data-operation="${singleOperation.id}"`)
  const row = explorer.slice(rowStart, explorer.indexOf('</li>', rowStart))
  assert.doesNotMatch(row, /data-service=/)
})

test('a filtered view round-trips through the query string, and an unfiltered one is clean', () => {
  // C-102. The page promises "every operation has a stable page you can share" — true of an
  // operation, false of a *view*. The encode/decode pair is what makes it true of a view, so it is a
  // pure function in `data/catalog.mts` and asserted here rather than left as component state.
  //
  // Every value that is catalogue data is read out of the catalogue. The only vocabularies named
  // literally are the ones the site owns and the catalogue cannot supply: the parameter keys, the
  // defect filter's two choices, and the sort orders.
  const providers = catalog().providers
  const multi = providers.find((provider) => visibleServices(provider).length > 1)
  assert.ok(multi, 'no connector publishes several services; this would not exercise the pair')
  const sample = multi.operations[0]

  const empty = selectors.emptyView()

  // An empty filter contributes no parameter, so the unfiltered URL is clean.
  assert.equal(selectors.encodeView(empty), '', 'the unfiltered view still writes a parameter')
  assert.deepEqual(selectors.decodeView(''), empty)
  assert.deepEqual(selectors.decodeView('?'), empty)

  const views = [
    empty,
    { ...empty, query: 'list' },
    { ...empty, query: sample.path },
    { ...empty, provider: multi.id },
    { ...empty, provider: multi.id, service: sample.service },
    { ...empty, risk: sample.risk },
    { ...empty, idempotency: sample.idempotency },
    { ...empty, defect: 'own' },
    { ...empty, defect: 'none' },
    { ...empty, sort: 'id' },
    { ...empty, sort: 'risk' },
    {
      query: sample.path,
      provider: multi.id,
      service: sample.service,
      risk: sample.risk,
      idempotency: sample.idempotency,
      defect: 'own',
      sort: 'risk',
    },
  ]

  for (const view of views) {
    const encoded = selectors.encodeView(view)
    assert.deepEqual(
      selectors.decodeView(encoded),
      view,
      `the view does not survive the round trip through \`?${encoded}\``
    )
    // One view, one string: re-encoding what was parsed cannot drift.
    assert.equal(selectors.encodeView(selectors.decodeView(encoded)), encoded)
  }

  // Only the fields that are set appear at all.
  assert.deepEqual(
    [...new URLSearchParams(selectors.encodeView({ ...empty, risk: sample.risk })).keys()],
    ['risk'],
    'a view with one filter set writes more than one parameter'
  )

  // Two routes to the same view produce the same string: the key order a caller happens to build
  // the object in is not part of the URL.
  assert.equal(
    selectors.encodeView({ ...empty, provider: multi.id, sort: 'id' }),
    selectors.encodeView({
      sort: 'id',
      defect: '',
      idempotency: '',
      risk: '',
      service: '',
      provider: multi.id,
      query: '',
    }),
    'the same view encodes to two different strings'
  )

  // Unknown or stale parameters are ignored, not fatal — a shared link outliving a rename degrades
  // to a wider view rather than to an error page.
  const full = views.at(-1)
  const encoded = selectors.encodeView(full)
  assert.deepEqual(selectors.decodeView(`${encoded}&nonesuch=1`), full)
  assert.deepEqual(selectors.decodeView('nonesuch=1'), empty)
  assert.deepEqual(
    selectors.decodeView('sort=nonesuch'),
    empty,
    'an unrecognised sort order survives parsing, so a stale link sorts by nothing'
  )
  assert.deepEqual(
    selectors.decodeView('defect=nonesuch'),
    empty,
    'an unrecognised defect filter survives parsing, so a stale link hides every operation'
  )

  // A filter value the catalogue no longer offers is dropped against the catalogue itself, which is
  // the half of "ignored, not fatal" that a pure parse cannot decide: a renamed connector must widen
  // the view, not empty it.
  const stale = { ...empty, provider: 'nonesuch', service: 'nonesuch', risk: 'nonesuch' }
  assert.deepEqual(
    selectors.narrowView(stale, providers),
    empty,
    'a link naming a connector, service or risk the catalogue no longer publishes filters everything away'
  )

  // A service its connector does not publish is dropped, and the connector is kept.
  const single = providers.find((provider) => visibleServices(provider).length === 0)
  assert.ok(single, 'every connector publishes a named service; this case is untested')
  assert.deepEqual(
    selectors.narrowView({ ...empty, provider: single.id, service: sample.service }, providers),
    { ...empty, provider: single.id },
    'a service the chosen connector does not publish survives and empties the list'
  )

  // What the catalogue does publish is left exactly alone.
  assert.deepEqual(selectors.narrowView(full, providers), full)
})

test('the operation list sorts by catalogue order, by id, and by declared risk', () => {
  const ops = operations(catalog())
  const ids = ops.map((operation) => operation.id)

  // Catalogue order is the default and it is meaningful — it is the order the module emits.
  assert.deepEqual(
    selectors.sortOperations(ops, 'catalog').map((operation) => operation.id),
    ids,
    'the default sort reorders the catalogue'
  )
  assert.deepEqual(ops.map((operation) => operation.id), ids, 'sorting mutated its input')

  assert.deepEqual(
    selectors.sortOperations(ops, 'id').map((operation) => operation.id),
    [...ids].sort(),
    'sorting by id does not order by id'
  )

  // The one ordering the catalogue cannot supply. JSON carries no notion of which tier is worse, and
  // alphabetical would put the worst tier in the middle, which is wrong without ever looking wrong.
  assert.deepEqual(selectors.RISK_ORDER, ['low', 'medium', 'high', 'destructive'])

  const tiers = [...new Set(ops.map((operation) => operation.risk))]
  for (const tier of tiers) {
    assert.ok(
      selectors.RISK_ORDER.includes(tier),
      `the catalogue publishes the risk tier \`${tier}\` and the declared order does not rank it`
    )
  }

  const byRisk = selectors.sortOperations(ops, 'risk')
  assert.equal(byRisk.length, ops.length, 'sorting by risk drops or duplicates operations')

  const ranks = byRisk.map((operation) => selectors.RISK_ORDER.indexOf(operation.risk))
  assert.deepEqual(ranks, [...ranks].sort((a, b) => a - b), 'sorting by risk is not in rank order')

  // Catalogue order is the tiebreaker inside a tier, so the default sort is never thrown away — it
  // is only grouped.
  for (const tier of tiers) {
    const same = (operation) => operation.risk === tier
    assert.deepEqual(
      byRisk.filter(same).map((operation) => operation.id),
      ops.filter(same).map((operation) => operation.id),
      `sorting by risk reorders within \`${tier}\`, losing catalogue order as the tiebreaker`
    )
  }

  // Every sort the URL can name is one the comparator answers.
  for (const sort of selectors.SORTS) {
    assert.equal(
      selectors.sortOperations(ops, sort).length,
      ops.length,
      `the sort \`${sort}\` is offered in the URL and not implemented`
    )
  }
})

test('changing a filter replaces the URL rather than pushing a history entry', () => {
  // C-102, and the reason it is asserted on the source: the failure is invisible on the page. A
  // pushed entry per keystroke means the back button walks back through a search instead of leaving
  // the explorer, and nothing about the rendered HTML shows it.
  const list = readFileSync(
    path.join(webRoot, '.vitepress', 'theme', 'components', 'OperationList.vue'),
    'utf-8'
  )

  assert.match(
    list,
    /replaceState/,
    'the operation list no longer replaces the URL when a filter changes'
  )
  assert.doesNotMatch(
    list,
    /pushState|router\.go\(/,
    'the operation list pushes a history entry for a filter change, so the back button walks back through every keystroke of a search'
  )
})

test('the explorer shows every multi-surface service and omits a single-surface default', () => {
  const document = catalog()
  const explorer = page('explorer.html')

  for (const provider of document.providers) {
    const services = visibleServices(provider)

    if (services.length === 0) {
      // A connector whose sole surface is `default` says nothing about services at all. Those cards
      // growing a service row would contradict the address it is elided from.
      assert.doesNotMatch(
        explorer,
        new RegExp(`data-service-of="${provider.id}"`),
        `\`${provider.id}\` addresses one surface and its card still lists a service`
      )
      continue
    }

    for (const service of services) {
      assert.match(
        explorer,
        new RegExp(`data-service-of="${provider.id}"[^>]*data-service="${service.name}"`),
        `the card for \`${provider.id}\` does not list its service \`${service.name}\``
      )
      assert.equal(
        selectors.serviceApiVersion(provider, service),
        service.api_version === provider.api_version ? null : service.api_version,
        `the version shown for \`${service.name}\` repeats or drops its connector's`
      )
    }

    // The count each service carries, and the version where it differs from its connector's.
    const start = explorer.indexOf(`id="${provider.id}"`)
    const card = explorer.slice(start, explorer.indexOf('</section>', start))
    for (const service of services) {
      const entry = card.slice(card.indexOf(`data-service="${service.name}"`))
      const rendered = text(entry.slice(0, entry.indexOf('</li>')))
      assert.ok(
        rendered.includes(String(service.operation_count)),
        `the service \`${service.name}\` does not show its operation count`
      )
      assert.ok(
        rendered.includes(selectors.serviceLabel(service.name)),
        `the service \`${service.name}\` does not show its presentation label`
      )
      const version = selectors.serviceApiVersion(provider, service)
      if (version) {
        assert.ok(
          rendered.includes(version),
          `the service \`${service.name}\` does not show the version that differs from its connector's`
        )
      }
    }
  }

  // An operation states its service exactly when its connector addresses more than one surface.
  for (const provider of document.providers) {
    const labelled = visibleServices(provider).length > 1
    for (const operation of provider.operations) {
      if (labelled) {
        assert.match(
          explorer,
          new RegExp(
            `data-operation="${operation.id}"[^>]*data-service="${operation.service}"|data-service="${operation.service}"[^>]*data-operation="${operation.id}"`
          ),
          `the row for \`${operation.id}\` does not say which service it belongs to`
        )
        const start = explorer.indexOf(`data-operation="${operation.id}"`)
        const row = explorer.slice(start, explorer.indexOf('</li>', start))
        assert.ok(
          text(row).includes(selectors.serviceLabel(operation.service)),
          `the row for \`${operation.id}\` does not show its service label`
        )
      } else {
        assert.doesNotMatch(
          explorer,
          new RegExp(`data-operation="${operation.id}"[^>]*data-service=`),
          `the row for \`${operation.id}\` names a service its connector does not publish`
        )
      }
    }
  }
})

test('a published service address is shown, and an absent one is shown as nothing', () => {
  const document = catalog()
  const explorer = page('explorer.html')

  const addresses = document.providers
    .flatMap((provider) => provider.services.map((service) => service.gid))
    .filter((gid) => gid !== null)
  assert.ok(addresses.length > 0, 'no service publishes an address; this would pass vacuously')

  const body = text(explorer)
  for (const address of addresses) {
    assert.ok(body.includes(address), `the explorer does not show the address \`${address}\``)
  }

  // Most services declare no authority and so have no address. Rendering that as a placeholder
  // would put a value on the page the catalogue does not publish.
  const absent = document.providers
    .flatMap((provider) => provider.services)
    .filter((service) => service.gid === null)
  assert.ok(absent.length > 0, 'every service publishes an address; the null case is untested')
  assert.doesNotMatch(
    explorer,
    />\s*null\s*</,
    'the explorer renders a null the catalogue does not publish'
  )
})

test('nothing about the catalogue is hand-maintained in the explorer sources', () => {
  const files = explorerSources()
  assert.ok(files.length > 0, 'no explorer sources were found; this test would pass vacuously')

  const sources = files.map((file) => ({ file, source: readFileSync(file, 'utf-8') }))
  for (const { file, value } of handMaintainedData(sources, catalogueValues(catalog()))) {
    assert.fail(
      `${file} names \`${value}\` — catalogue data hand-written into the site is the failure this project exists to correct`
    )
  }
})

// C-205, the first of the guard's two halves. The gate was red on `main` because the guard read the
// sources as raw text: `data/catalog.data.mts` says "reloads the dev server" in a comment about
// VitePress, Postmark declares a service called `server`, and the two collided.
//
// Asserted against the tree as it stands rather than against a fixture alone, because the point of
// the story is that the real comment is allowed to stay English. The first assertion is what keeps
// this honest: if nobody ever writes a catalogue word in prose again, the tolerance is untested and
// this says so instead of passing.
test('a catalogue name used as an English word in prose is not hand-maintained data', () => {
  const values = catalogueValues(catalog())
  const sources = explorerSources().map((file) => ({ file, source: readFileSync(file, 'utf-8') }))

  const written = sources.flatMap(({ file, source }) =>
    [...values]
      .filter((value) => namesValue(source, value))
      .map((value) => `${path.relative(webRoot, file)}: ${value}`)
  )
  assert.ok(
    written.length > 0,
    'no explorer source writes a catalogue name at all, so the tolerance this test exists for is untested'
  )

  assert.deepEqual(
    handMaintainedData(sources, values),
    [],
    `the guard reads prose as data — each of these is a sentence, not a value: ${written.join(', ')}`
  )

  // And in the small, so the mechanism is pinned rather than inferred from the tree. Every one of
  // the catalogue's one-word service names, in the three comment forms the sources are written in.
  const words = [...values].filter((value) => /^[a-z]+$/.test(value)).join(' ')
  assert.ok(words.includes('server'), 'the catalogue no longer declares a one-word service name')
  const fixtures = [
    { file: 'data/notes.mts', source: `// ${words}\n/* ${words} */\nexport const rows = []\n` },
    {
      file: 'theme/Card.vue',
      source: [
        `<script setup>\n// ${words}\n</script>`,
        `<template>\n  <!-- ${words} -->\n  <p>{{ label }}</p>\n</template>`,
        `<style scoped>\n/* ${words} */\n.card { color: red; }\n</style>`,
      ].join('\n\n'),
    },
    { file: 'explorer.md', source: `# The explorer\n\n<!-- ${words} -->\n\n<CatalogExplorer />\n` },
  ]
  assert.deepEqual(handMaintainedData(fixtures, values), [])
})

// C-205, the other half, and the one that matters more: the narrowing above must not have turned
// the guard off. A comment is exempt because it renders nothing — everything a reader can see is
// still read as raw text, in every language the sources are written in.
test('the guard still catches catalogue data hand-written into the explorer', () => {
  const document = catalog()
  const values = catalogueValues(document)
  const provider = document.providers.find((entry) => entry.operations.length > 0)
  const { id, base_url: url } = provider
  const operation = provider.operations[0].id

  const caught = (file, source) =>
    handMaintainedData([{ file, source }], values).map((hit) => hit.value)
  const finds = (file, source, value) =>
    assert.ok(
      caught(file, source).includes(value),
      `the guard no longer catches \`${value}\` hand-written into ${file}`
    )

  // A literal in code, on a line whose comment is stripped: the removal is surgical, not by line.
  finds('data/catalog.mts', `export const first = '${id}' // the first connector\n`, id)
  // A base URL, which contains the `//` a line-wise comment sweep would have cut it at.
  finds('data/catalog.mts', `const base = '${url}'\n`, url)
  // Rendered text and a bound attribute in a Vue template, and a literal in its script block.
  finds('theme/Card.vue', `<template>\n  <p>${operation}</p>\n</template>\n`, operation)
  finds('theme/Card.vue', `<template>\n  <a href="/operations/${operation}">go</a>\n</template>\n`, operation)
  finds('theme/Card.vue', `<script setup>\nconst only = '${id}'\n</script>\n`, id)
  // A style block, where a hard-coded id would be a selector or generated content.
  finds('theme/Card.vue', `<style>\n.card[data-provider='${id}'] { color: red; }\n</style>\n`, id)
  // And the body of a page, which is all rendered.
  finds('explorer.md', `# The explorer\n\nStart with ${id}.\n`, id)

  // A value glued into a longer identifier is still a value: the word boundary ends at a capital,
  // exactly where an identifier's own word does.
  finds('data/catalog.mts', `const ${id}Ticket = 1\n`, id)

  // The exemption is the comment and nothing wider: a value in code is caught on the same line that
  // an exempt one sits in a comment.
  const surgical = caught('data/catalog.mts', `const first = '${id}' // and ${operation}\n`)
  assert.ok(surgical.includes(id), 'a literal beside a comment is no longer read')
  assert.ok(!surgical.includes(operation), 'the comment beside it is read after all')

  // And the two narrowings do not overlap into a third: the word rule exempts a longer *word*, not
  // a longer line, so it cannot be used to smuggle a value past the guard by suffixing it.
  const services = document.providers.flatMap((entry) => entry.services)
  assert.ok(
    services.some((entry) => entry.name === 'delivery'),
    'no connector declares the service `delivery`, so the case below is stale'
  )
  assert.ok(!caught('data/catalog.mts', 'delivery_id: FieldSelector | null\n').includes('delivery'))
  assert.ok(caught('data/catalog.mts', "const only = 'delivery'\n").includes('delivery'))

  // The message still names the file and the value, because it is the only thing the author sees.
  const hand = { file: path.join(webRoot, 'data', 'catalog.mts'), source: `'${id}'` }
  assert.deepEqual(handMaintainedData([hand], values)[0], { file: 'data/catalog.mts', value: id })
})

// C-142. The sibling of the test above: that one keeps catalogue *data* out of the components, this
// one keeps the *site framework* out of them.
//
// Named as an identifier rather than as prose because the story names it as one.
//
// The measurement the story rests on is that the coupling was two symbols wide — `withBase` in five
// components and `inBrowser` in one — so the components were already a tier that happened to import
// its host. This asserts the boundary now that it is explicit: a component may import Vue, a sibling
// component, and the catalogue's typed contract, and nothing else.
//
// Read off the sources rather than off the rendered page, because the failure is invisible in the
// output: a component importing `withBase` renders exactly the same HTML here and simply cannot be
// mounted anywhere else.
test('no_component_imports_the_site_framework', () => {
  const sources = componentSources()
  assert.ok(sources.length > 0, 'no components were found; this test would pass vacuously')

  const rel = (file) => path.relative(webRoot, file)
  const imports = new Map(
    sources.map((file) => [file, importedModules(readFileSync(file, 'utf-8'))])
  )

  // The framework itself. `withBase` is supplied through `provide`/`inject` by whoever mounts these,
  // which for this site is `.vitepress/theme/index.mts`; the default is identity.
  const coupled = sources.filter((file) =>
    imports.get(file).some((module) => module === 'vitepress' || module.startsWith('vitepress/'))
  )
  assert.deepEqual(
    coupled.map(rel),
    [],
    `${coupled.length} component(s) import VitePress directly, so they can only ever be mounted in this site: ${coupled.map(rel).join(', ')}`
  )

  // And nothing else either — a component that reaches for its own data, or for the filesystem,
  // cannot be attached anywhere, whatever it imports it from.
  const ALLOWED = /^(vue|\.\/[A-Za-z]+\.vue|(?:\.\.\/)+data\/catalog\.mts)$/
  for (const file of sources) {
    for (const module of imports.get(file)) {
      assert.match(
        module,
        ALLOWED,
        `${rel(file)} imports \`${module}\` — a component takes what it renders as props or injected context, and may otherwise import only Vue, a sibling component, or the catalogue's typed contract`
      )
    }
  }
})

test('a card fact holding several values can break between them', () => {
  // C-100, rework. The hosts cell renders one `<code>` per host with no whitespace between them, so
  // the markup offers no soft-wrap opportunity and the run is one unbreakable inline box. That was
  // survivable while the explorer was one 609px column; widening it to two ~424px columns pushed the
  // run off the *page* — 29px of horizontal overflow at 1280, against 0 at the merge base.
  //
  // Asserted on the built artefacts rather than on a screenshot: the class has to reach every card
  // that needs it, and the rule has to survive in the emitted stylesheet. A layout regression here is
  // silent, so the test names the mechanism.
  const providers = catalog().providers
  const multi = providers.filter((provider) => provider.hosts.length > 1)
  assert.ok(
    multi.length,
    'no connector publishes more than one host, so this test no longer covers anything — if that is a real change in the catalogue, delete it'
  )

  const html = page('explorer.html')
  const cells = html.match(/class="card__hosts"/g) ?? []
  assert.equal(
    cells.length,
    providers.length,
    `${cells.length} of ${providers.length} cards carry the wrapping hosts cell — every card needs it, because which connector grows a second host is the catalogue's business and not the site's`
  )

  const assets = path.join(distDir, 'assets')
  const css = readdirSync(assets)
    .filter((entry) => entry.endsWith('.css'))
    .map((entry) => readFileSync(path.join(assets, entry), 'utf-8'))
    .join('\n')
  const rule = css.match(/\.card__hosts[^{]*\{([^}]*)\}/)
  assert.ok(rule, 'the `.card__hosts` rule is gone from the built stylesheet')
  assert.match(
    rule[1],
    /flex-wrap:\s*wrap/,
    `\`.card__hosts\` no longer wraps (${rule[1]}) — the hosts run becomes one unbreakable box again and escapes the page at 1280px`
  )
})

test('nothing in the explorer sets a floor under its own width', () => {
  // C-100, follow-up. Three separate regressions had one cause: a flex or grid item's automatic
  // minimum size is its *min-content*, so a control, a row or a card silently refuses to go below
  // its longest unbreakable run and pushes its container instead.
  //
  //   - a `<select>`'s min-content is its widest option, so eight filters could never share a row
  //   - a grid item's min-content held every operation row open at its longest request path, which
  //     scrolled the whole page sideways on a phone
  //   - the provider card's header held a 314px floor under a card, which capped the grid at three
  //     columns however wide the page got
  //
  // Each is released by `min-width: 0` or by letting the run wrap. The rules are asserted in the
  // emitted stylesheet because a layout regression here is silent: the page still renders, it just
  // renders wrong, and only at some viewport widths.
  const css = stylesheet()

  for (const [selector, property] of [
    ['.filters__field', /min-width:\s*0/],
    ['.row', /min-width:\s*0/],
    ['.card__head', /flex-wrap:\s*wrap/],
  ]) {
    // The lookahead keeps `.row` off `.row__head` and `.filters__field` off `--wide`; without it
    // the assertion would silently drift onto a neighbouring rule if the emitted order changed.
    const rule = css.match(new RegExp(`\\${selector}(?![\\w-])[^{]*\\{([^}]*)\\}`))
    assert.ok(rule, `the \`${selector}\` rule is gone from the built stylesheet`)
    assert.match(
      rule[1],
      property,
      `\`${selector}\` no longer releases its automatic minimum size (${rule[1]}) — whatever it contains sets a floor under the layout again`
    )
  }
})

// A tool contract is the page's most information-dense block, and it was rendered as bare text plus
// an unhighlighted `JSON.stringify`. These assertions pin the two properties that make it readable
// and that a refactor would silently lose: the safety fields carry a *derived* tone, and the schema
// is tokenised rather than dumped.
//
// Read out of the built HTML, so this also holds the block to the suite's standing rule that the
// content survives without JavaScript.
test('a tool contract renders its safety fields as toned chips and its schema highlighted', () => {
  const document = catalog()
  const ops = document.core.operations.filter((entry) => entry.tool_spec)
  assert.ok(ops.length > 0, 'no core operation carries a tool spec; this test would pass vacuously')

  let checked = 0
  for (const operation of ops) {
    const html = page('core', 'operations', `${operation.name}.html`)
    if (!html.includes('Tool contract')) continue
    checked += 1

    // The tone is derived from the value, never passed in — so a risk level cannot be rendered calm
    // on one page and alarming on another.
    const tones = [...html.matchAll(/class="chip chip--([a-z]+)"[^>]*>([^<]+)/g)]
    assert.ok(
      tones.length >= 2,
      `${operation.name} renders its tool contract without chips — the fields are bare text again`
    )
    for (const [, tone, value] of tones) {
      assert.match(tone, /^(alarming|cautionary|reassuring|neutral)$/, `unknown tone on \`${value}\``)
    }

    // An unrecognised value must stay neutral rather than being guessed at: a wrong colour on a
    // safety field reads as an assurance nobody made.
    const riskTone = tones.find(([, , value]) => value.trim() === operation.tool_spec.risk)
    assert.ok(riskTone, `${operation.name} does not render its declared risk as a chip`)

    // The schema is tokenised, not dumped. Keys and punctuation are present in any JSON object.
    assert.match(
      html,
      /tok tok--key/,
      `${operation.name}'s input schema is not highlighted — it is a raw JSON dump again`
    )
    assert.ok(
      html.includes('aria-label="Schema format"'),
      `${operation.name} offers no JSON/YAML choice`
    )
  }

  assert.ok(checked > 0, 'no page rendered a tool contract; the selector above is stale')
})

// ---------------------------------------------------------------------------------------------
// C-206 — a withheld credential is not the same fact as a vendor that requires none.
//
// An operation listing no credentials is in one of two opposite situations. Either a real credential
// exists and this repository cannot hold it safely yet, so the reader waits and the fail-closed 401
// is the correct outcome; or the vendor requires nothing, the unauthenticated call is the working
// call, and the reader has nothing to do at all. The catalogue published one sentence for both, so
// the explorer told a visitor that a working public endpoint was disabled for their protection.
//
// The catalogue answers it now, as a `notes` entry. Asserted against the shipped document where it
// can be — the invariants below hold of whatever is in it — and against fixtures for the rest,
// because no connector declares a public operation yet and a test that waited for one would pass
// vacuously until it did.
// ---------------------------------------------------------------------------------------------

test('a fact that is not a defect is published apart from the reasons an operation fails', () => {
  const document = catalog()

  // `works` is `issues.length === 0` and a note is not an issue — the property that lets a fifth
  // code be added without moving the boolean every consumer already filters on.
  for (const operation of operations(document)) {
    assert.equal(
      operation.status.works,
      operation.status.issues.length === 0,
      `\`${operation.id}\` disagrees with itself about whether it works`
    )
    for (const note of selectors.notes(operation)) {
      assert.match(note.scope, /^(catalog|provider|operation)$/, `\`${note.code}\` has no scope`)
      assert.ok(note.summary.length > 0, `\`${note.code}\` publishes no sentence to render`)
      assert.ok(!('params' in note), `\`${note.code}\` grew a parameter list a note has no use for`)
    }
  }

  // The catalogue always publishes the key. The second case is the build-order window only — the
  // committed catalogue is regenerated by a full build, so between an emitter change and that build
  // the site reads a document older than itself and must stay buildable rather than throw on every
  // operation at once. It is a tolerance in one selector, never a shape a consumer should expect.
  const operation = (status) => ({ id: 'fixture', credentials: [], status })
  assert.deepEqual(selectors.notes(operation({ works: false, issues: [], notes: [] })), [])
  assert.deepEqual(selectors.notes(operation({ works: false, issues: [] })), [])

  // The two situations, told apart. Both list no credential and only one of them is waiting on
  // anything, which is precisely what a reader could not see before.
  const withheld = operation({
    works: false,
    issues: [
      {
        code: 'withheld',
        scope: 'provider',
        summary: 'a credential cannot be held safely yet',
        params: [],
      },
    ],
  })
  const needsNone = operation({
    works: true,
    issues: [],
    notes: [{ code: 'needs-none', scope: 'operation', summary: 'this vendor requires none' }],
  })

  assert.equal(selectors.notes(withheld).length, 0)
  assert.equal(selectors.notes(needsNone).length, 1)
  assert.notDeepEqual(
    [withheld.status.works, selectors.notes(withheld)],
    [needsNone.status.works, selectors.notes(needsNone)],
    'a withheld credential and a vendor that needs none are still indistinguishable here'
  )

  // The sentence a page shows comes from the catalogue and never from this site: a note carries its
  // own summary exactly as an issue does.
  assert.equal(selectors.notes(needsNone)[0].summary, 'this vendor requires none')
})

test('the operation page says what to supply from the catalogue, and claims nothing it is not told', () => {
  const detail = readFileSync(
    path.join(webRoot, '.vitepress', 'theme', 'components', 'OperationDetail.vue'),
    'utf-8'
  )

  // The false sentence is the one this story is about: "no safe credential configuration … live
  // calls are disabled" is true of a withheld credential and false of a public endpoint. It may
  // still be rendered, but no longer for an operation the catalogue publishes a note for.
  const fallback = detail.match(/<p v-if="([^"]+)" class="op__note">\s*No safe credential/)
  assert.ok(fallback, 'the operation page no longer has a credential fallback this test can read')
  assert.match(
    fallback[1],
    /notes|clear/,
    'the credential fallback still fires for an operation the catalogue says needs no credential'
  )

  // And every built page renders exactly the notes the catalogue publishes for it — no more, which
  // would be invention, and no fewer, which would be the conflation again.
  const document = catalog()
  for (const operation of operations(document)) {
    const html = page('operations', `${operation.id}.html`)
    const marker = html.match(/data-notes="(\d+)"/)
    assert.ok(marker, `the page for \`${operation.id}\` does not report how many notes it rendered`)
    assert.equal(
      Number(marker[1]),
      selectors.notes(operation).length,
      `the page for \`${operation.id}\` renders a different number of notes than the catalogue has`
    )
  }
})

// ---------------------------------------------------------------------------------------------
// C-408. What a component may say about a field it was not given.
//
// A second consumer mounts these components over a *thinner* document than `public/catalog.json` —
// one that publishes no `auth`, no `credentials`, no `method`/`path`, no `flux` and no `base_url`.
// Every one of those absences used to render as a statement about the **connector**: a red "not
// configured" on every card, "live calls are disabled" on every operation, two empty chips.
//
// Read off the component sources and off the built pages, never by mounting a component: the site
// has exactly one dependency and a test that imported Vue to render an SFC would add one it does
// not declare. So the *decision* lives in `data/catalog.mts`, where it is a pure function a fixture
// can pin, and these tests assert that each component routes through it.
// ---------------------------------------------------------------------------------------------

/** One component's source, as text. */
function component(name) {
  return readFileSync(path.join(webRoot, '.vitepress', 'theme', 'components', name), 'utf-8')
}

/** A component's `<template>` block — the half a reader sees. */
function markup(source) {
  const match = source.match(/<template>([\s\S]*)<\/template>/)
  assert.ok(match, 'a component with no template')
  return match[1]
}

/** The declarations of one CSS rule of a component, by selector. */
function rule(source, selector) {
  const style = source.match(/<style[^>]*>([\s\S]*?)<\/style>/)
  assert.ok(style, 'a component with no style block')
  const match = style[1].match(new RegExp(`\\${selector}\\s*\\{([^}]*)\\}`))
  assert.ok(match, `the component no longer has a \`${selector}\` rule`)
  return match[1]
}

/**
 * The `v-if` / `v-else-if` / `v-else` on the element that renders `needle`.
 *
 * The guard is the whole question here: a false sentence is not fixed by deleting it — it is still
 * true of the connector it was written for — but by firing it only when the source said so.
 */
function guard(source, needle) {
  const template = markup(source)
  const at = template.indexOf(needle)
  assert.notEqual(at, -1, `no element renders \`${needle}\` any more`)
  const open = template.lastIndexOf('<', at)
  const tag = template.slice(open, template.indexOf('>', open) + 1)
  const match = tag.match(/\sv-(if|else-if|else)(?:="([^"]*)")?/)
  assert.ok(match, `the element rendering \`${needle}\` is not guarded at all: ${tag}`)
  return { kind: match[1], expression: match[2] ?? '' }
}

test('a field a source did not publish is told apart from one the connector does not have', () => {
  // The predicate. An empty collection is a *published* answer — the connector has none — and only
  // an absent or null field means the source declined to say.
  assert.equal(selectors.published([]), true)
  assert.equal(selectors.published(''), true)
  assert.equal(selectors.published(0), true)
  assert.equal(selectors.published(null), false)
  assert.equal(selectors.published(undefined), false)

  const provider = (fields) => ({ id: 'fixture', ...fields })
  const operation = (fields) => ({ id: 'fixture', ...fields })
  const auth = (schemes) => ({ schemes, credentials: [], default: [] })

  // Auth: three outcomes where a bare `schemes.length` had two.
  assert.deepEqual(selectors.providerAuth(provider({ auth: auth(['fixture']) })).schemes, [
    'fixture',
  ])
  assert.deepEqual(selectors.providerAuth(provider({ auth: auth([]) })).schemes, [])
  assert.equal(selectors.providerAuth(provider({ auth: null })), null)
  assert.equal(selectors.providerAuth(provider({})), null)

  // Credentials: the same three, and the middle one is the fact worth stating in full.
  assert.deepEqual(selectors.operationCredentials(operation({ credentials: [['fixture']] })), [
    ['fixture'],
  ])
  assert.deepEqual(selectors.operationCredentials(operation({ credentials: [] })), [])
  assert.equal(selectors.operationCredentials(operation({ credentials: null })), null)
  assert.equal(selectors.operationCredentials(operation({})), null)

  // And the signature, which reads the first line of the Flux: an unpublished module has no first
  // line, and asking for one used to throw rather than render nothing.
  assert.equal(
    selectors.signature(operation({ flux: 'fn fixture() -> Any {\n}\n' })),
    'fn fixture() -> Any {'
  )
  assert.equal(selectors.signature(operation({})), null)

  // The sentence is site vocabulary in one place, so four components cannot drift into four of them.
  assert.ok(selectors.UNPUBLISHED.length > 0)
})

test('a catalogue that omits auth does not put a red claim on every connector card', () => {
  const card = component('ProviderCard.vue')

  // The claim that must survive untouched: a connector whose catalogue publishes auth and lists no
  // scheme really is not configured, and that is worth showing in the danger colour.
  const configured = guard(card, 'not configured')
  assert.equal(
    configured.kind,
    'else-if',
    'the danger-coloured "not configured" is no longer a branch of a three-way choice'
  )
  assert.match(
    configured.expression,
    /\bauth\b/,
    'the card still says "not configured" without first asking whether the source published auth'
  )
  assert.match(rule(card, '.card__warn'), /--vp-c-danger-1/, 'the real claim has been softened')

  // The branch this story adds: nothing about the connector, and not in red.
  const unpublished = guard(card, 'UNPUBLISHED')
  assert.equal(unpublished.kind, 'else')
  assert.doesNotMatch(
    rule(card, '.card__unpublished'),
    /danger/,
    'a field the source did not publish is still painted as a defect'
  )

  // The card reads the guarded auth, never the raw field — an unpublished `auth` has no `schemes`
  // to take a length of, so the old expression did not merely mislead, it threw.
  assert.doesNotMatch(
    markup(card),
    /provider\.auth/,
    'the card still reaches into `provider.auth` directly'
  )
  assert.match(card, /providerAuth\(/, 'the card no longer resolves its auth through the catalogue')
})

test('an operation a source publishes no credentials for is not told live calls are disabled', () => {
  const detail = component('OperationDetail.vue')

  // Same shape as the card: the sentence is correct for a withheld credential and stays, but only
  // for a document that published the credential set at all.
  const disabled = guard(detail, 'No safe credential configuration')
  assert.match(
    disabled.expression,
    /^credentials &&/,
    'the page still claims live calls are disabled for a source that published no credentials'
  )
  assert.match(
    detail,
    /operationCredentials\(/,
    'the page no longer resolves its credentials through the catalogue'
  )
  assert.doesNotMatch(
    markup(detail),
    /operation\.credentials/,
    'the page still reaches into `operation.credentials` directly'
  )

  // And it says which of the two it is looking at.
  assert.match(markup(detail), /UNPUBLISHED/, 'the page renders no unpublished branch at all')
})

test('a request shape a source did not publish is omitted rather than rendered as an empty chip', () => {
  for (const name of ['OperationDetail.vue', 'OperationRow.vue']) {
    const source = component(name)
    for (const field of ['method', 'path']) {
      const chip = guard(source, `operation.${field} }}`)
      assert.equal(chip.kind, 'if', `${name} renders \`${field}\` unconditionally`)
      assert.match(
        chip.expression,
        new RegExp(`published\\(operation\\.${field}\\)`),
        `${name} renders \`${field}\` without asking whether the source published it`
      )
    }
  }

  // The list's search reads the path too, and a needle typed against an unpublished one must match
  // nothing rather than throw.
  assert.doesNotMatch(
    component('OperationList.vue'),
    /operation\.path\.toLowerCase/,
    'the search still calls a method on a path the source may not have published'
  )
})

test('semantic effects are visible through the shared policy-tone chip', () => {
  const detail = component('OperationDetail.vue')
  assert.match(detail, /import SpecChip from '.\/SpecChip\.vue'/)
  assert.match(detail, /operation\.semantic_effects/)
  assert.match(detail, /Semantic effects/)

  const chip = component('SpecChip.vue')
  assert.match(chip, /ALARMING = \[[^\]]*'money'[^\]]*'delete'/)
  assert.match(chip, /CAUTIONARY = \[[^\]]*'send_external'/)

  const capture = page('operations', 'stripe-payment-intent-capture.html')
  assert.match(
    capture,
    /class="chip chip--alarming"[^>]*>money</,
    'a real charge is not rendered with the alarming policy tone'
  )
  const cancel = text(page('operations', 'stripe-payment-intent-cancel.html'))
  assert.match(cancel, /Semantic effects\s*none/, 'an empty semantic set is hidden rather than stated')
})

test('the full catalogue renders exactly what it publishes and says nothing about a source', () => {
  // The additive half. `public/catalog.json` publishes every field, so nothing in this build takes
  // an unpublished branch — and the two sentences that were conflated still appear exactly as often
  // as the catalogue's own data says they should.
  const document = catalog()
  const explorer = text(page('explorer.html'))

  const configured = document.providers.filter((provider) => provider.auth.schemes.length === 0)
  assert.ok(configured.length > 0, 'no connector publishes an empty scheme list, so the case is stale')
  assert.equal(
    [...explorer.matchAll(/not configured/g)].length,
    configured.length,
    `"not configured" is rendered for a different number of connectors than the ${configured.length} the catalogue publishes no scheme for`
  )
  for (const provider of document.providers) {
    if (provider.auth.schemes.length) {
      assert.ok(
        explorer.includes(provider.auth.schemes.join(', ')),
        `the card for \`${provider.id}\` no longer states the schemes the catalogue publishes`
      )
    }
  }

  const withheld = operations(document).filter(
    (operation) => operation.credentials.length === 0 && selectors.notes(operation).length === 0
  )
  assert.ok(withheld.length > 0, 'no operation withholds a credential, so this case is untested')
  for (const operation of withheld) {
    assert.ok(
      text(page('operations', `${operation.id}.html`)).includes(
        'No safe credential configuration is available'
      ),
      `\`${operation.id}\` withholds a credential and the page no longer says so`
    )
  }

  // And no page in the built site claims a field was not published, because every one of them was.
  const pages = []
  const walk = (dir) => {
    for (const entry of readdirSync(dir)) {
      const full = path.join(dir, entry)
      if (statSync(full).isDirectory()) walk(full)
      else if (full.endsWith('.html')) pages.push(full)
    }
  }
  walk(distDir)
  assert.ok(pages.length > 0, 'the site was not built — run `npm run build` before `npm test`')
  for (const file of pages) {
    assert.ok(
      !text(readFileSync(file, 'utf-8')).includes(selectors.UNPUBLISHED),
      `${path.relative(distDir, file)} says a field was not published, of a catalogue that publishes every one`
    )
  }
})
