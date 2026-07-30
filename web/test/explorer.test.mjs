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
 * It is vocabulary from the address grammar: an operation naming no service belongs to it, no
 * provider may declare it, and it is elided from every published address. The consequence the
 * explorer has to honour is that nothing renders it and no filter offers it — a card listing it, or
 * an option selecting it, would name something no address contains.
 */
const RESERVED_SERVICE = 'default'

/** The services a provider publishes under a name of their own, in catalogue order. */
function namedServices(provider) {
  return provider.services.filter((service) => service.name !== RESERVED_SERVICE)
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

/** Every file under the explorer's own sources — where hand-maintained data would have to live. */
function explorerSources() {
  const roots = [
    path.join(webRoot, 'data'),
    path.join(webRoot, 'operations'),
    path.join(webRoot, '.vitepress', 'theme'),
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
  return files.filter(existsSync)
}

test('the site ships the generated catalogue at the path VitePress serves', () => {
  const document = catalog()

  assert.equal(document.schema_version, 2)
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

test('the custom domain serves the catalogue and versioned specs from the origin root', () => {
  assert.equal(readFileSync(path.join(webRoot, 'public', 'CNAME'), 'utf-8').trim(), 'flux.codewandler.org')
  const config = readFileSync(path.join(webRoot, '.vitepress', 'config.mts'), 'utf-8')
  assert.match(config, /const base = '\/'/)
  assert.doesNotMatch(config, /const base = '\/flux-connectors\/'/)
})

test('an operation that owns a defect says so wherever it appears', () => {
  const document = catalog()
  const explorer = page('explorer.html')
  const owners = operations(document).filter((operation) => ownIssues(operation).length > 0)

  assert.ok(owners.length > 0, 'no operation owns a defect; this assertion would pass vacuously')

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

test('the service filter is a facet of the catalogue and narrows to the chosen connector', () => {
  const providers = catalog().providers

  const published = [
    ...new Set(providers.flatMap((provider) => namedServices(provider).map((s) => s.name))),
  ]
  assert.ok(published.length > 0, 'no connector publishes a named service; this would pass vacuously')

  // With no connector chosen, every service the catalogue publishes is on offer — and the reserved
  // one never is, at any narrowing.
  assert.deepEqual(selectors.serviceFacet(providers), published)
  for (const provider of providers) {
    assert.ok(
      !selectors.serviceFacet(providers, provider.id).includes(RESERVED_SERVICE),
      'the service filter offers the reserved service, which names no address'
    )
  }

  // Choosing a connector narrows the options to that connector's own, in catalogue order.
  for (const provider of providers) {
    assert.deepEqual(
      selectors.serviceFacet(providers, provider.id),
      namedServices(provider).map((service) => service.name),
      `choosing \`${provider.id}\` does not narrow the service options to its own`
    )
  }

  // The narrowing is worth having only if the catalogue actually varies: at least one connector
  // publishes several services and at least one publishes none of its own.
  const several = providers.filter((provider) => namedServices(provider).length > 1)
  const single = providers.filter((provider) => namedServices(provider).length === 0)
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

test('a filtered view round-trips through the query string, and an unfiltered one is clean', () => {
  // C-102. The page promises "every operation has a stable page you can share" — true of an
  // operation, false of a *view*. The encode/decode pair is what makes it true of a view, so it is a
  // pure function in `data/catalog.mts` and asserted here rather than left as component state.
  //
  // Every value that is catalogue data is read out of the catalogue. The only vocabularies named
  // literally are the ones the site owns and the catalogue cannot supply: the parameter keys, the
  // defect filter's two choices, and the sort orders.
  const providers = catalog().providers
  const multi = providers.find((provider) => namedServices(provider).length > 1)
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
  const single = providers.find((provider) => namedServices(provider).length === 0)
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

test('the explorer shows the services a connector publishes, and never the reserved one', () => {
  const document = catalog()
  const explorer = page('explorer.html')

  for (const provider of document.providers) {
    const services = namedServices(provider)

    if (services.length === 0) {
      // A connector with one surface says nothing about services at all. Fifteen cards growing a
      // row reading `default` would contradict the address it is elided from.
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
    const labelled = namedServices(provider).length > 1
    for (const operation of provider.operations) {
      if (labelled) {
        assert.match(
          explorer,
          new RegExp(
            `data-operation="${operation.id}"[^>]*data-service="${operation.service}"|data-service="${operation.service}"[^>]*data-operation="${operation.id}"`
          ),
          `the row for \`${operation.id}\` does not say which service it belongs to`
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
  const document = catalog()

  const forbidden = new Set()
  for (const provider of document.providers) {
    forbidden.add(provider.id)
    forbidden.add(provider.vendor)
    forbidden.add(provider.base_url)
    provider.hosts.forEach((host) => forbidden.add(host))
    provider.auth.credentials.forEach((credential) => forbidden.add(credential.name))
    // A service name and a published address are catalogue data like any other. The reserved name
    // is the one exception, and only because it is not data: it is grammar the site has to know
    // about in order to render nothing for it.
    for (const service of provider.services) {
      if (service.name !== RESERVED_SERVICE) forbidden.add(service.name)
      if (service.gid) forbidden.add(service.gid)
    }
    for (const operation of provider.operations) {
      forbidden.add(operation.id)
      operation.status.issues.forEach((issue) => forbidden.add(issue.code))
    }
  }

  for (const file of explorerSources()) {
    const source = readFileSync(file, 'utf-8')
    for (const token of forbidden) {
      assert.ok(
        !source.includes(token),
        `${path.relative(webRoot, file)} names \`${token}\` — catalogue data hand-written into the site is the failure this project exists to correct`
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
