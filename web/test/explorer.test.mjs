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

/** The issues an operation owns itself, as opposed to the ones it inherits. */
function ownIssues(operation) {
  return operation.status.issues.filter((issue) => issue.scope === 'operation')
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

test('nothing about the catalogue is hand-maintained in the explorer sources', () => {
  const document = catalog()

  const forbidden = new Set()
  for (const provider of document.providers) {
    forbidden.add(provider.id)
    forbidden.add(provider.vendor)
    forbidden.add(provider.base_url)
    provider.hosts.forEach((host) => forbidden.add(host))
    provider.auth.credentials.forEach((credential) => forbidden.add(credential.name))
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
