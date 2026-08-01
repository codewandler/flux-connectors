// The host page's contract with the host (C-239).
//
// Until this file existed, `crates/connectors-api/src/index.html` was ~260 lines of JavaScript that
// nothing could execute, and `AGENTS.md` requires a failing-first test for a behavioural change.
// C-234's security review measured the consequence: of its 16 mutations, M15 — drawing the developer
// sign-in unconditionally, `if (status.dev)` → `if (true)` — stayed **green**, and its acceptance
// recorded the reason as *"needs a JS harness this crate does not have — its own story"*. This is
// that story. `the developer sign-in is drawn only when the host says it is in dev mode`, below, is
// red under M15.
//
// # The shape is `web/test/explorer.test.mjs`'s, deliberately
//
// `node --test`; assertions against the **served bytes** and against the **emitted stylesheet**,
// which is how the site catches a layout regression without a screenshot; guards written as
// properties over what is there rather than as a list of values to keep in step.
//
// It is **not** under `web/`. That tree is a public GitHub Pages site forbidden by C-147 to collect
// a credential, and its single-dependency property is deliberate; a harness for the page whose whole
// job is collecting one does not belong in it. This tree has exactly one dependency of its own.
//
// # The one dependency, and why the page is executed rather than grepped
//
// `happy-dom`. Three of the five checks here are about what the page *does* — a button that appears,
// a click that issues a POST and does not navigate — and grepping a source for `if (status.dev)`
// would pass for a page that had been rewritten around it. A headless browser is deliberately not
// taken: it is a different decision with its own cost, and every property named below is reachable
// without one.
//
// JavaScript evaluation is opt-in in happy-dom v20 and warns that a VM context is not a sandbox.
// The only code this file evaluates is this repository's own committed page, read off disk, so the
// warning is suppressed here and nowhere else.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { Window } from 'happy-dom'

const uiRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const crateRoot = path.resolve(uiRoot, '..')
const pagePath = path.join(crateRoot, 'src', 'index.html')

/** The origin the host binds. Any other value here would make "did it navigate?" untestable. */
const ORIGIN = 'http://localhost:8787/'

// ---------------------------------------------------------------------------------------------
// Reading what is served
// ---------------------------------------------------------------------------------------------

/**
 * The bytes an operator's browser receives.
 *
 * `crates/connectors-api/src/ui.rs` compiles the page in with `include_str!`, so the file on disk
 * *is* the built output — there is no bundler between them today. That is asserted rather than
 * assumed, because it is the assumption every check in this file rests on: a page served from
 * somewhere else would leave the whole suite passing about a file nobody reads.
 *
 * C-238 adds a bundler beside this directory. When it does, this function moves to the emitted
 * bundle and nothing below it changes.
 */
function servedPage() {
  const ui = readFileSync(path.join(crateRoot, 'src', 'ui.rs'), 'utf-8')
  assert.match(
    ui,
    /include_str!\("index\.html"\)/,
    'src/ui.rs no longer serves src/index.html, so this suite tests a file the host does not send'
  )
  return readFileSync(pagePath, 'utf-8')
}

/** The `<style>` the page emits, as text. */
function stylesheetText(source) {
  const blocks = [...source.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/g)].map((block) => block[1])
  assert.ok(blocks.length > 0, 'the page emits no stylesheet at all')
  return blocks.join('\n')
}

/** The page's script, as text. */
function scriptText(source) {
  const blocks = [...source.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)].map((block) => block[1])
  assert.ok(blocks.length > 0, 'the page carries no script at all')
  return blocks.join('\n')
}

// ---------------------------------------------------------------------------------------------
// Driving it
//
// A stubbed `fetch`, because the page's whole conversation with the host is `fetch`. Every call is
// recorded with its method, which is what makes "an auth state change is a POST" an assertion about
// behaviour rather than about the source.
// ---------------------------------------------------------------------------------------------

/**
 * The page, loaded and settled, with `fetch` answering from `routes`.
 *
 * `/auth/status` is answered from `status` — it is the one route every state goes through, and the
 * three sign-in states below differ in nothing else.
 */
async function render(status, routes = {}) {
  const window = new Window({
    url: ORIGIN,
    settings: { enableJavaScriptEvaluation: true, suppressInsecureJavaScriptEnvironmentWarning: true },
  })

  const calls = []
  window.fetch = async (input, init = {}) => {
    const url = String(input)
    const method = init.method ?? 'GET'
    // The request body is recorded too (C-237): "invalid JSON never reaches the vendor" is a claim
    // about what went out, not about whether anything did. Recorded only when there *is* one, so a
    // call with no body still deep-equals `{ url, method }` and the guards above are untouched.
    const call = { url, method }
    if (init.body !== undefined) call.body = init.body
    calls.push(call)
    const route = url === '/auth/status' ? status : routes[url]
    // A route may answer as a value — every caller before C-237 does — or as a function of the
    // method, which is what lets one path answer a `GET` and a `DELETE` differently.
    const answered = typeof route === 'function' ? route(method, init.body) : route
    const status_code = answered?.__status ?? 200
    const body = answered?.__status === undefined ? (answered ?? null) : (answered.body ?? null)
    return {
      ok: status_code < 400,
      status: status_code,
      text: async () => JSON.stringify(body),
    }
  }

  window.document.write(servedPage())
  await window.happyDOM.waitUntilComplete()

  const settle = async () => {
    await window.happyDOM.waitUntilComplete()
  }
  return { window, document: window.document, calls, settle, close: () => window.happyDOM.close() }
}

/** Every rendered button whose label matches, across the whole document. */
function buttons(document, label) {
  return [...document.querySelectorAll('button')].filter((button) => label.test(button.textContent))
}

/** A signed-out status, which is where both doors are drawn. */
const signedOut = (rest = {}) => ({ configured: true, dev: false, signed_in: false, ...rest })

// ---------------------------------------------------------------------------------------------
// C-234's M15, closed by name
// ---------------------------------------------------------------------------------------------

test('the developer sign-in is drawn only when the host says it is in dev mode', async () => {
  // The mutation this test exists for: with `if (status.dev)` removed, the button below is drawn on
  // an ordinary host and the first assertion is red. That is the whole of M15.
  const plain = await render(signedOut({ dev: false }))
  try {
    assert.deepEqual(
      buttons(plain.document, /DEVELOPER/).map((button) => button.textContent),
      [],
      'the host does not report dev mode and the page offers the developer sign-in anyway — the route is not even in the router, so this button 404s, and it reads as a way in on a host that has none'
    )
  } finally {
    await plain.close()
  }

  const dev = await render(signedOut({ dev: true }))
  try {
    const offered = buttons(dev.document, /DEVELOPER/)
    assert.equal(offered.length, 1, 'dev mode draws no developer sign-in, or draws more than one')
    const [button] = offered

    // C-234's requirement that it cannot be mistaken for the real door: it is the *secondary*
    // action, and `ghost` is the class that says so. The stylesheet check below is the other half —
    // the class only means something while a rule gives it a look of its own.
    assert.ok(
      button.classList.contains('ghost'),
      'the developer sign-in is drawn as the primary action, beside a real Google sign-in it must not be confused with'
    )

    // And what it does. A POST, because `SameSite=Lax` carries a session cookie on a cross-site
    // top-level *GET* and not on a POST; and no navigation, because a link is exactly what that
    // rules out.
    const before = dev.window.location.href
    button.click()
    await dev.settle()

    const auth = dev.calls.filter((call) => call.url === '/auth/dev')
    assert.deepEqual(
      auth,
      [{ url: '/auth/dev', method: 'POST' }],
      'activating the developer sign-in does not POST to /auth/dev'
    )
    assert.equal(
      dev.window.location.href,
      before,
      'activating the developer sign-in navigates — a state change reached by a top-level GET is the request `SameSite=Lax` still carries the cookie on'
    )
  } finally {
    await dev.close()
  }
})

// ---------------------------------------------------------------------------------------------
// The three states an operator lands in
// ---------------------------------------------------------------------------------------------

test('each of the three sign-in states renders its own surface', async () => {
  // Unconfigured — the first-run path. `tests/host.rs` covers it at the status-code level and
  // nothing covered it at the content level: a 200 carrying an empty page would have passed there.
  const SETUP = 'GOOGLE_CLIENT_ID=… GOOGLE_CLIENT_SECRET=…'
  const unconfigured = await render({ configured: false, dev: false, signed_in: false, setup: SETUP })
  try {
    const detail = unconfigured.document.querySelector('#detail')
    assert.match(detail.textContent, /not set up/, 'an unconfigured host does not say sign-in is unset')
    assert.equal(
      unconfigured.document.querySelector('#detail pre')?.textContent,
      SETUP,
      'an unconfigured host does not show the operator what to set'
    )
    assert.deepEqual(
      [...unconfigured.document.querySelectorAll('button')],
      [],
      'an unconfigured host offers a sign-in button, which reaches a route it never registered'
    )
  } finally {
    await unconfigured.close()
  }

  // Signed out — the doors, and only the ones that exist.
  const doors = await render(signedOut())
  try {
    assert.deepEqual(
      buttons(doors.document, /Sign in with Google/).length,
      1,
      'a configured host offers no Google sign-in'
    )
    assert.match(doors.document.querySelector('#who').textContent, /not signed in/)

    // The Google door *is* a navigation, and deliberately: it is the OAuth2 authorization request,
    // a top-level GET to the identity provider's flow. Asserted here so that the POST rule above is
    // demonstrably a rule about state changes rather than a rule about every button.
    buttons(doors.document, /Sign in with Google/)[0].click()
    await doors.settle()
    assert.equal(doors.window.location.href, `${ORIGIN}auth/signin`.replace('//auth', '/auth'))
  } finally {
    await doors.close()
  }

  // Signed in — the catalogue, scoped to that account's tenant.
  const catalogue = [
    { id: 'fixture', vendor: 'Fixture', operation_count: 3, callable_operations: 3, wiring: 'wired' },
  ]
  const inside = await render(
    { configured: true, dev: false, signed_in: true, account: { email: 'operator@example.test', tenant: 'fixture-tenant' } },
    { '/v1/connectors': catalogue }
  )
  try {
    assert.ok(
      inside.calls.some((call) => call.url === '/v1/connectors'),
      'a signed-in operator is not shown the catalogue'
    )
    assert.match(inside.document.querySelector('#counts').textContent, /1 connectors · 3 operations/)
    assert.match(inside.document.querySelector('#list').textContent, /Fixture/)
    assert.match(
      inside.document.querySelector('#who').textContent,
      /operator@example\.test · tenant fixture-tenant/,
      'a signed-in operator is not told which account and tenant they are operating as'
    )
  } finally {
    await inside.close()
  }
})

// ---------------------------------------------------------------------------------------------
// The two guards that were held only by a comment
// ---------------------------------------------------------------------------------------------

/**
 * Script comments removed, string literals kept — `web/test/explorer.test.mjs`'s scanner, and here
 * for the same reason it is there.
 *
 * The page's own comment at the `<pre>` says *"`textContent` on a <pre>, never innerHTML"*, and the
 * one below it explains why a state change is a POST. A raw grep for `innerHTML` therefore reports
 * the sentence promising not to use it, so the guard would be red on arrival and would have to be
 * weakened into something that no longer catches the real thing.
 *
 * A quote that opens no string makes the scanner read on as string content rather than dropping it:
 * every ambiguity fails towards keeping text, which is the safe direction for a guard.
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

/** Markup comments removed. */
const withoutMarkupComments = (source) => source.replace(/<!--[\s\S]*?-->/g, ' ')

/** Style comments removed. CSS has only the one form. */
const withoutStyleComments = (source) => source.replace(/\/\*[\s\S]*?\*\//g, ' ')

/**
 * What a page contributes to a browser: its markup, its script and its style, with the prose
 * written *about* them removed. An HTML page is three languages, so each block is read as its own.
 */
function renderedSource(file, source) {
  const script = ['.mjs', '.cjs', '.js', '.mts', '.cts', '.ts']
  const extension = path.extname(file)
  if (script.includes(extension)) return withoutScriptComments(source)
  if (extension === '.css') return withoutStyleComments(source)
  if (extension !== '.html' && extension !== '.vue') return source

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
 * Every source the operator page is built from: the page itself, and — once C-238 lands a bundler
 * beside this directory — every component it imports.
 *
 * Derived from the tree rather than listed, so the component the console grows tomorrow is covered
 * with no edit here. That is the property the guard below needs to be worth having.
 */
function pageSources() {
  const files = [pagePath]
  const walk = (dir) => {
    if (!existsSync(dir)) return
    for (const entry of readdirSync(dir)) {
      const full = path.join(dir, entry)
      if (statSync(full).isDirectory()) walk(full)
      else if (['.html', '.vue', '.mjs', '.js', '.mts', '.ts'].includes(path.extname(full))) files.push(full)
    }
  }
  walk(path.join(uiRoot, 'src'))
  return files
}

/**
 * The sinks that turn a string into markup.
 *
 * One family, not a list of pet hates: each parses its argument as HTML, so each is the same
 * one-server-change-away XSS the page's comment promises to avoid. `v-html` is Vue's, and belongs
 * here before a component exists rather than after one does.
 */
const MARKUP_SINKS = [/\binnerHTML\b/, /\bouterHTML\b/, /\binsertAdjacentHTML\b/, /\bv-html\b/]

test('the page builds every node from text, never from markup', () => {
  const sources = pageSources()
  assert.ok(sources.length > 0, 'no page source was found; this guard would pass vacuously')

  for (const file of sources) {
    const rendered = renderedSource(file, readFileSync(file, 'utf-8'))
    for (const sink of MARKUP_SINKS) {
      assert.doesNotMatch(
        rendered,
        sink,
        `${path.relative(crateRoot, file)} assigns through ${sink.source} — this page renders server-composed strings, and one that renders them as markup is one server change away from being an XSS sink`
      )
    }
  }

  // Both halves of the scanner, pinned in the small. Without the first, a rewrite that tolerated
  // more would go unnoticed; without the second, a rewrite that tolerated everything would.
  const prose = `<script>\n// never innerHTML, and no v-html either\n/* not outerHTML */\nconst a = 1\n</script>`
  const code = `<script>\nnode.innerHTML = untrusted\n</script>`
  assert.ok(
    !MARKUP_SINKS.some((sink) => sink.test(renderedSource('page.html', prose))),
    'the guard reads a sentence about innerHTML as a use of it, so the page cannot explain its own rule'
  )
  assert.ok(
    MARKUP_SINKS.some((sink) => sink.test(renderedSource('page.html', code))),
    'the guard no longer catches an actual assignment to innerHTML'
  )

  // And the page really does state the rule in prose, so the tolerance above is exercised by the
  // tree and not only by the fixture.
  assert.match(
    scriptText(servedPage()),
    /never innerHTML/,
    'the page no longer explains why it builds nodes from text, so the comment-stripping this guard needs is untested against the real source'
  )
})

/** The two routes that change auth state. `/auth/signin` is deliberately not one — see below. */
const STATE_CHANGING = ['/auth/signout', '/auth/dev']

test('an auth state change is a POST, never a link', async () => {
  // The `SameSite=Lax` property, until now a comment at the sign-out handler. Lax withholds the
  // session cookie from a cross-site POST and *sends* it on a cross-site top-level GET, so a link
  // or a `location` assignment to either route below is a one-click sign-out — or, in dev, a
  // one-click session — from any page on the internet.

  // Behaviour first: each route is reached, and reached by POST.
  const dev = await render(signedOut({ dev: true }))
  try {
    buttons(dev.document, /DEVELOPER/)[0].click()
    await dev.settle()
    assert.deepEqual(dev.calls.filter((call) => call.url === '/auth/dev'), [
      { url: '/auth/dev', method: 'POST' },
    ])
  } finally {
    await dev.close()
  }

  const inside = await render(
    { configured: true, dev: false, signed_in: true, account: { email: 'operator@example.test', tenant: 'fixture-tenant' } },
    { '/v1/connectors': [] }
  )
  try {
    const out = buttons(inside.document, /Sign out/)
    assert.equal(out.length, 1, 'a signed-in operator is offered no way out')
    const before = inside.window.location.href
    out[0].click()
    await inside.settle()
    assert.deepEqual(inside.calls.filter((call) => call.url === '/auth/signout'), [
      { url: '/auth/signout', method: 'POST' },
    ])
    assert.equal(inside.window.location.href, before, 'signing out navigates rather than posting')
  } finally {
    await inside.close()
  }

  // And structurally, because a second call site could be added without a test for it: every
  // mention of either route in the page's code is the argument of a `fetch`.
  const script = withoutScriptComments(scriptText(servedPage()))
  for (const route of STATE_CHANGING) {
    const quoted = `'${route}'`
    let found = 0
    for (let at = script.indexOf(quoted); at !== -1; at = script.indexOf(quoted, at + 1)) {
      found += 1
      assert.match(
        script.slice(0, at).trimEnd().slice(-16),
        /\bfetch\($/,
        `${route} is reached other than by \`fetch(\` — a state change on a route \`SameSite=Lax\` still carries the cookie to`
      )
    }
    assert.ok(found > 0, `the page no longer reaches ${route} at all, so this guard passes vacuously`)
  }

  // The counter-case, which is what keeps the rule above a rule about *state changes*.
  // `/auth/signin` starts the OAuth2 authorization request: a top-level GET is what that is, and it
  // establishes nothing on its own. A guard that forbade every navigation would have to make an
  // unexplained exception for it; this one does not, and says so here.
  assert.match(
    script,
    /location\.href\s*=\s*'\/auth\/signin'/,
    'the Google door no longer navigates, so the distinction this guard draws is untested'
  )
})

// ---------------------------------------------------------------------------------------------
// The emitted stylesheet
// ---------------------------------------------------------------------------------------------

test('the emitted stylesheet keeps the developer sign-in a secondary action', async () => {
  // `web/test/explorer.test.mjs` asserts against the built stylesheet because a layout regression is
  // otherwise only visible in a screenshot. The same applies here, and it is not cosmetic: C-234
  // requires the developer sign-in to be unmistakable for the real one, and the *only* thing
  // separating them visually is that `ghost` gives it a look of its own. Deleting the rule leaves
  // the class in the markup, every assertion above green, and two identical primary buttons on the
  // page — one of which authenticates nobody.
  const dev = await render(signedOut({ dev: true }))
  try {
    const [fake] = buttons(dev.document, /DEVELOPER/)
    const [real] = buttons(dev.document, /Sign in with Google/)
    assert.ok(fake && real, 'dev mode does not draw both doors, so they cannot be compared')

    const styleOf = (node) => {
      const computed = dev.window.getComputedStyle(node)
      return { background: computed.backgroundColor, border: computed.borderColor, color: computed.color }
    }
    const secondary = styleOf(fake)
    const primary = styleOf(real)

    assert.notDeepEqual(
      secondary,
      primary,
      'the developer sign-in is drawn exactly like the real one — an operator cannot tell the fake account from their own'
    )
    assert.notEqual(
      secondary.background,
      primary.background,
      'the developer sign-in carries the primary action fill'
    )
  } finally {
    await dev.close()
  }

  // The rule itself, in the emitted stylesheet, so a deletion names itself rather than surfacing as
  // two colours that happen to match.
  assert.match(
    withoutStyleComments(stylesheetText(servedPage())),
    /button\.ghost\s*\{[^}]*background:\s*transparent/,
    'the stylesheet no longer draws `button.ghost` as an unfilled secondary action'
  )
})

// ---------------------------------------------------------------------------------------------
// The console (C-237)
//
// Six properties about what the page does with what the host already sends it. Every fixture below
// is the *shape* of `/v1/connectors/{id}`; `crates/connectors-api/tests/wiring.rs` is what holds the
// host to producing it, and `wiring_vocabulary.rs` is what holds the two to one vocabulary.
// ---------------------------------------------------------------------------------------------

/** A signed-in status. Everything in this section happens behind it. */
const signedIn = (rest = {}) => ({
  configured: true,
  dev: false,
  signed_in: true,
  account: { email: 'operator@example.test', tenant: 'fixture-tenant' },
  ...rest,
})

/** How many operations the fixture connector ships. Large enough that an N+1 is a measurement. */
const OPERATION_COUNT = 30

/**
 * The operations of a connector, as `ConnectorView::operations` carries them.
 *
 * Every field here is one the host fills from the catalogue entry it already has in hand. The page
 * having them *is* the N+1 fix: before C-237 `operations[]` carried `id`, `requires`, `requirement`
 * and `callable`, and the page fetched the other five one request at a time.
 */
const fixtureOperations = () =>
  Array.from({ length: OPERATION_COUNT }, (_, index) => ({
    id: `fixture-op-${index}`,
    tool: `fixture.op.${index}`,
    service: index % 3 === 0 ? 'management' : 'default',
    description: `Fixture operation ${index}`,
    risk: index % 2 ? 'low' : 'medium',
    idempotency: 'idempotent',
    hosts: ['api.fixture.test'],
    requires: [['fixture.api_key']],
    requirement: 'declared',
    callable: true,
  }))

/** One connector, as `GET /v1/connectors/{id}` serves it. */
const fixtureConnector = (over = {}) => ({
  id: 'fixture',
  vendor: 'Fixture',
  description: 'A connector that exists only in this harness.',
  authority: 'fixture.test',
  base_url: 'https://api.fixture.test',
  operation_count: OPERATION_COUNT,
  operations: fixtureOperations(),
  wiring: 'wired',
  callable_operations: OPERATION_COUNT,
  credentials: [
    {
      name: 'fixture.api_key',
      leaf: 'api_key',
      placement: 'header:Authorization',
      needs_username: false,
      address: 'fixture-tenant/fixture.test/default/api_key',
      stored: true,
    },
  ],
  settings: [],
  config_choices: [],
  ...over,
})

/** One operation, as `GET /v1/operations/{id}` serves it — the expanded view, with the Flux. */
const fixtureDetail = (index, over = {}) => ({
  id: `fixture-op-${index}`,
  provider: 'fixture',
  service: index % 3 === 0 ? 'management' : 'default',
  description: `Fixture operation ${index}`,
  risk: index % 2 ? 'low' : 'medium',
  idempotency: 'idempotent',
  hosts: ['api.fixture.test'],
  credentials: [['fixture.api_key']],
  tool: `fixture.op.${index}`,
  flux: `op fixture.op.${index} { }`,
  input_schema: { type: 'object', properties: { ticket_id: { type: 'string' } } },
  ...over,
})

/** Every route a fixture connector needs, detail included, so nothing 404s into a crash. */
function fixtureRoutes(connector = fixtureConnector(), extra = {}) {
  const routes = { '/v1/connectors': [connector], [`/v1/connectors/${connector.id}`]: connector }
  for (let index = 0; index < OPERATION_COUNT; index += 1) {
    routes[`/v1/operations/fixture-op-${index}`] = fixtureDetail(index)
  }
  return { ...routes, ...extra }
}

/** The requests the page made for a *single* operation's detail. The N+1's own measurement. */
const detailFetches = (calls) =>
  calls.filter((call) => /^\/v1\/operations\/[^/]+$/.test(call.url) && call.method === 'GET')

/** The connector rows the left rail is currently showing. */
const rail = (document) => [...document.querySelectorAll('#list .conn')]

/** The operation rows the detail pane is currently showing. */
const rows = (document) => [...document.querySelectorAll('.op')]

/** The operation row whose tool name is `tool`. */
const row = (document, tool) => rows(document).find((node) => node.textContent.includes(tool))

/** Type into a control the page listens to, the way a person does. */
async function type(page, control, text) {
  control.value = text
  control.dispatchEvent(new page.window.Event('input', { bubbles: true }))
  await page.settle()
}

// ---------------------------------------------------------------------------------------------

test('opening a connector fetches no operation detail, and expanding one fetches exactly that one', async () => {
  // The measurement C-237 exists for. `show()` used to run
  // `Promise.all(c.operations.map(o => api('GET', '/v1/operations/' + o.id)))` — one request per
  // operation *in* the connector, 30 for this fixture and ~30 for the largest shipped one, to read
  // three fields. C-212 put `requires` and `callable` on `operations[]` so this would not be needed
  // and the page kept doing it; C-237 puts the other five there too, so the list costs nothing and
  // only an expansion costs a request.
  const page = await render(signedIn(), fixtureRoutes())
  try {
    rail(page.document)[0].click()
    await page.settle()

    assert.deepEqual(
      detailFetches(page.calls).map((call) => call.url),
      [],
      `opening a connector fetched ${detailFetches(page.calls).length} operation details to render a list the host already sent whole — every field those responses carry is on \`operations[]\``
    )
    assert.equal(
      rows(page.document).length,
      OPERATION_COUNT,
      'the operations were not rendered at all, so costing nothing proves nothing'
    )

    // And the other half: an expansion *is* a request, because `flux` and `input_schema` are the
    // two things the list deliberately does not carry.
    row(page.document, 'fixture.op.7').click()
    await page.settle()
    assert.deepEqual(
      detailFetches(page.calls).map((call) => call.url),
      ['/v1/operations/fixture-op-7'],
      'expanding one operation fetched something other than exactly that operation'
    )
  } finally {
    await page.close()
  }
})

test('operations are grouped by service, and idempotency and hosts are rendered', async () => {
  // `service` is the addressing level C-49 established, and the dimension `explorer-ux.md` says the
  // public explorer is missing too. `OperationView` returned all three and the page rendered none
  // of them.
  const page = await render(signedIn(), fixtureRoutes())
  try {
    rail(page.document)[0].click()
    await page.settle()

    const groups = [...page.document.querySelectorAll('.service')]
    assert.deepEqual(
      groups.map((group) => group.dataset.service).sort(),
      ['default', 'management'],
      'the operations are one flat list — the service each belongs to is on every row and is not shown'
    )
    let grouped = 0
    for (const group of groups) {
      const service = group.dataset.service
      for (const node of group.querySelectorAll('.op')) {
        grouped += 1
        const index = Number(node.dataset.operation.replace('fixture-op-', ''))
        assert.equal(
          index % 3 === 0 ? 'management' : 'default',
          service,
          `\`fixture.op.${index}\` is grouped under \`${service}\`, which is not its service`
        )
      }
    }
    assert.equal(grouped, OPERATION_COUNT, 'some operations are in no service group at all')

    const detail = page.document.querySelector('#detail').textContent
    assert.match(detail, /idempotent/, "an operation's idempotency is discarded")
    assert.match(detail, /api\.fixture\.test/, 'the hosts an operation reaches are discarded')
  } finally {
    await page.close()
  }
})

test('the connector list can be searched by connector, by vendor and by operation id', async () => {
  // 54 connectors and ~299 operations in one flat unsorted rail. The three things a person actually
  // types are asserted separately, because a search matching only the vendor name is the one that
  // gets written and the one that fails the operator looking for `ticket-list`.
  const zendesk = fixtureConnector({
    id: 'zendesk',
    vendor: 'Zendesk',
    description: 'Support ticketing.',
    operation_count: 1,
    callable_operations: 1,
    operations: [{ ...fixtureOperations()[0], id: 'zendesk-ticket-list', tool: 'zendesk.ticket.list' }],
  })
  const stripe = fixtureConnector({
    id: 'stripe',
    vendor: 'Stripe',
    description: 'Payments.',
    operation_count: 1,
    callable_operations: 1,
    operations: [{ ...fixtureOperations()[0], id: 'stripe-charge-list', tool: 'stripe.charge.list' }],
  })
  const page = await render(signedIn(), { '/v1/connectors': [zendesk, stripe] })
  try {
    const search = page.document.querySelector('#search')
    assert.ok(search, 'the rail has no search box, so 54 connectors are found by scrolling')

    const shown = () => rail(page.document).map((node) => node.textContent)
    assert.equal(shown().length, 2, 'the rail did not render both connectors to begin with')

    await type(page, search, 'zendesk')
    assert.equal(shown().length, 1, 'searching a connector id narrows to no single connector')
    assert.match(shown()[0], /Zendesk/)

    await type(page, search, 'Stripe')
    assert.equal(shown().length, 1, 'searching a vendor name narrows to no single connector')
    assert.match(shown()[0], /Stripe/)

    await type(page, search, 'ticket-list')
    assert.equal(
      shown().length,
      1,
      'searching an operation id finds nothing — the operator who knows the operation but not the vendor is left scrolling'
    )
    assert.match(shown()[0], /Zendesk/)

    await type(page, search, '')
    assert.equal(shown().length, 2, 'clearing the search does not restore the rail')
  } finally {
    await page.close()
  }
})

test('the rail can be narrowed to the connectors that still need setup', async () => {
  // *"Which of these still need setup"* is the operator's real question, and `wiring` already
  // answers it. The filter is keyed on the host's own tokens for the same reason the sentences are —
  // `wiring_vocabulary.rs` holds the page to every variant the host can send.
  const ready = fixtureConnector({ id: 'ready', vendor: 'Ready', wiring: 'wired' })
  const unset = fixtureConnector({
    id: 'unset',
    vendor: 'Unset',
    wiring: 'not-wired',
    callable_operations: 0,
  })
  const page = await render(signedIn(), { '/v1/connectors': [ready, unset] })
  try {
    const [needs] = buttons(page.document, /needs setup/i)
    assert.ok(needs, 'the rail cannot be narrowed to the connectors an operator still has work on')
    needs.click()
    await page.settle()
    assert.deepEqual(
      rail(page.document).map((node) => node.textContent.replace(/\d+\/?\d*/g, '').trim()),
      ['Unset'],
      'narrowing to the connectors that need setup keeps the ones that do not'
    )
  } finally {
    await page.close()
  }
})

test('an operation whose credential is withheld is never offered as ready', async () => {
  // C-235's distinction, at the unit that has it. `requirement: 'no-credential'` and
  // `requirement: 'no-credential-required'` both arrive with an empty `requires`, and the page
  // rendered the empty list into the sentence "needs " with nothing after it. An operator reading a
  // blank requirement reasonably concludes there is nothing to supply, which is exactly the
  // conclusion C-235 exists to stop them drawing about freshdesk.
  const connector = fixtureConnector({
    operation_count: 2,
    callable_operations: 1,
    wiring: 'partly-wired',
    credentials: [],
    operations: [
      {
        ...fixtureOperations()[0],
        id: 'fixture-public',
        tool: 'fixture.public',
        requires: [],
        requirement: 'no-credential-required',
        callable: true,
      },
      {
        ...fixtureOperations()[1],
        id: 'fixture-withheld',
        tool: 'fixture.withheld',
        requires: [],
        requirement: 'no-credential',
        callable: false,
      },
    ],
  })
  const page = await render(signedIn(), {
    '/v1/connectors': [connector],
    '/v1/connectors/fixture': connector,
  })
  try {
    rail(page.document)[0].click()
    await page.settle()

    const withheld = row(page.document, 'fixture.withheld')
    assert.ok(withheld, 'the withheld operation was not rendered')
    assert.doesNotMatch(
      withheld.textContent,
      /needs\s*$|needs\s{2,}/,
      'a withheld operation renders "needs" with nothing after it — the requirement list is empty because the credential is withheld, not because nothing is needed'
    )
    assert.match(
      withheld.textContent,
      /withheld|not held|cannot be held/i,
      'a withheld operation does not say its credential is withheld, so it reads as one nobody got round to configuring'
    )

    const open = row(page.document, 'fixture.public')
    assert.doesNotMatch(
      open.textContent,
      /withheld|cannot be held/i,
      'a positively-public operation is described as withheld — the two states differ by one word and mean opposite things about whether the connector works'
    )
  } finally {
    await page.close()
  }
})

test('a field the host did not publish reads as unpublished, never as a fact about the connector', async () => {
  // C-408's rule, on the other surface that reads a catalogue. A source that does not publish
  // `hosts` is not a connector that reaches no host, and a source that does not publish a
  // description is not a connector without one. The public explorer learned this from
  // flux-exchange's console rendering "not configured" in red on every card; this page has the same
  // mistake available and no second consumer to find it.
  const connector = fixtureConnector({
    description: null,
    operation_count: 1,
    callable_operations: 1,
    operations: [
      {
        ...fixtureOperations()[0],
        id: 'fixture-thin',
        tool: 'fixture.thin',
        description: null,
        hosts: null,
        idempotency: null,
      },
    ],
  })
  const page = await render(signedIn(), {
    '/v1/connectors': [connector],
    '/v1/connectors/fixture': connector,
  })
  try {
    rail(page.document)[0].click()
    await page.settle()

    const detail = page.document.querySelector('#detail')
    assert.doesNotMatch(
      detail.textContent,
      /\bnull\b|\bundefined\b/,
      'an unpublished field reached the page as the literal `null`, which is a rendering of the absence rather than a reading of it'
    )
    assert.match(
      row(page.document, 'fixture.thin').textContent,
      /not published/i,
      'an operation whose source published no description renders as though it simply has none'
    )

    // And the counter-case, which is what keeps this a rule about *absence* rather than a blanket
    // apology: a published field is rendered, not hedged.
    const full = await render(signedIn(), fixtureRoutes())
    try {
      rail(full.document)[0].click()
      await full.settle()
      assert.doesNotMatch(
        row(full.document, 'fixture.op.1').textContent,
        /not published/i,
        'a fully published operation is reported as unpublished'
      )
    } finally {
      await full.close()
    }
  } finally {
    await page.close()
  }
})

test('a stored credential can be removed from the page, and invalid parameters never reach the vendor', async () => {
  // Two holes in the same surface. `DELETE /v1/credentials/{provider}/{credential}` has existed
  // since C-203 and nothing on the page could reach it, so an operator could store a credential and
  // not take it back. And the parameter editor was a single-line `<input value="{}">` whose contents
  // went out unparsed, so a typo was diagnosed by the vendor as a 400 about a document it never got.
  const connector = fixtureConnector()
  const page = await render(signedIn(), fixtureRoutes(connector))
  try {
    rail(page.document)[0].click()
    await page.settle()

    const forget = buttons(page.document, /Remove|Forget|Delete/i)
    assert.ok(forget.length >= 1, 'a stored credential cannot be removed through the page')
    forget[0].click()
    await page.settle()
    assert.deepEqual(
      page.calls.filter((call) => call.method === 'DELETE').map((call) => call.url),
      ['/v1/credentials/fixture/fixture.api_key'],
      'removing a credential does not DELETE it at the address the host serves it from'
    )

    // The parameter editor: a body that is not JSON is refused here.
    row(page.document, 'fixture.op.7').click()
    await page.settle()
    const params = page.document.querySelector('#play textarea')
    assert.ok(params, 'the parameter editor is not a textarea, so a real body cannot be typed into it')

    await type(page, params, '{ "ticket_id": }')
    buttons(page.document, /^Send$/)[0].click()
    await page.settle()

    assert.deepEqual(
      page.calls.filter((call) => call.url.endsWith('/execute')),
      [],
      'invalid JSON was sent to the vendor, which then diagnosed a document the operator never wrote'
    )
    assert.match(
      page.document.querySelector('#play').textContent,
      /not valid JSON/i,
      'invalid JSON is refused silently, so the operator is left with a button that does nothing'
    )
  } finally {
    await page.close()
  }
})

test('a dry run shows the request without sending it, and a refusal is shown as written', async () => {
  // C-145's seam, reached from the page. `crates/connectors-api/tests/dry_run.rs` is what holds the
  // *route* to naming the unbound field, its service and the operation; this is what holds the page
  // to two things it could each get wrong on its own — rehearsing must not execute, and a refusal
  // must not be flattened into "something went wrong".
  const connector = fixtureConnector()
  const rehearsal = {
    operation: 'fixture-op-7',
    tool: 'fixture.op.7',
    request: {
      method: 'GET',
      url: 'https://api.fixture.test/v1/tickets/42',
      headers: { authorization: 'Bearer {fixture.api_key}' },
      body: null,
    },
    credentials: [
      {
        credential: 'fixture.api_key',
        reference: '{fixture.api_key}',
        place: 'header',
        target: 'authorization',
        prefix: 'Bearer ',
      },
    ],
  }
  const page = await render(
    signedIn(),
    fixtureRoutes(connector, { '/v1/operations/fixture-op-7/dry-run': rehearsal })
  )
  try {
    rail(page.document)[0].click()
    await page.settle()
    row(page.document, 'fixture.op.7').click()
    await page.settle()

    const [dry] = buttons(page.document, /^Dry run$/)
    assert.ok(dry, 'the page offers no way to see the request before sending it')
    dry.click()
    await page.settle()

    assert.deepEqual(
      page.calls.filter((call) => call.url.endsWith('/execute')),
      [],
      'a dry run sent the call — the one thing it exists not to do'
    )
    const panel = page.document.querySelector('#play').textContent
    assert.match(panel, /https:\/\/api\.fixture\.test\/v1\/tickets\/42/, 'the rehearsed URL is not shown')
    assert.match(panel, /authorization/, 'the rehearsed headers are not shown')
    assert.match(
      panel,
      /no stored value was read|Not sent/,
      'the panel does not say that nothing was sent, so it reads like a response'
    )
  } finally {
    await page.close()
  }

  // And the refusal, which is the half worth having. `MissingConfig` names the field and its
  // service; a page that replaced it with its own wording would be answering a different question.
  const REFUSAL =
    '`zendesk-ticket-show` needs configuration field `subdomain` for service `default`, which is not bound'
  const refused = await render(
    signedIn(),
    fixtureRoutes(connector, {
      '/v1/operations/fixture-op-7/dry-run': { __status: 400, body: { error: REFUSAL } },
    })
  )
  try {
    rail(refused.document)[0].click()
    await refused.settle()
    row(refused.document, 'fixture.op.7').click()
    await refused.settle()
    buttons(refused.document, /^Dry run$/)[0].click()
    await refused.settle()
    assert.match(
      refused.document.querySelector('#play').textContent,
      /subdomain/,
      'the refusal was flattened — the field an operator has to bind is the whole content of it'
    )
  } finally {
    await refused.close()
  }
})

test('the response is legible: status, headers and body are distinguished and JSON is formatted', async () => {
  // `exec::Outcome::content` is the JSON-encoded `{status, headers, body}` flux's `http.request`
  // makes canonical (C-403), and the page wrote the whole document into a `<pre>` verbatim. The
  // redactor's output passes through unchanged — it is what stops a vendor echoing a token onto this
  // surface — so the check below is that the *shape* is read, never that the text is rewritten.
  const connector = fixtureConnector()
  const outcome = {
    tool: 'fixture.op.7',
    is_error: false,
    content: JSON.stringify({
      status: 201,
      headers: { 'content-type': 'application/json', 'x-request-id': 'req_fixture' },
      body: { id: 'tkt_1', subject: 'A ticket', tags: ['a', 'b'] },
    }),
  }
  const page = await render(
    signedIn(),
    fixtureRoutes(connector, { '/v1/operations/fixture-op-7/execute': outcome })
  )
  try {
    rail(page.document)[0].click()
    await page.settle()
    row(page.document, 'fixture.op.7').click()
    await page.settle()
    buttons(page.document, /^Send$/)[0].click()
    await page.settle()

    const panel = page.document.querySelector('#play')
    const status = panel.querySelector('.response-status')
    const headers = panel.querySelector('.response-headers')
    const body = panel.querySelector('.response-body')
    assert.ok(status && headers && body, 'the response is still one undifferentiated block of text')
    assert.match(status.textContent, /201/, 'the status is not shown')
    assert.match(headers.textContent, /x-request-id/, 'the headers are not shown')
    assert.match(
      body.textContent,
      /\n {2}"id": "tkt_1"/,
      'a JSON body is not formatted, so it is read as one line'
    )

    // The redactor's own output is text and stays text. A response that is *not* the canonical
    // document is shown whole rather than dropped — the alternative is a page that hides what it
    // could not parse, which is how an operator stops trusting the panel.
    const REDACTED = 'HTTP 200 Authorization: Bearer [REDACTED]'
    const opaque = await render(
      signedIn(),
      fixtureRoutes(connector, {
        '/v1/operations/fixture-op-7/execute': {
          tool: 'fixture.op.7',
          is_error: false,
          content: REDACTED,
        },
      })
    )
    try {
      rail(opaque.document)[0].click()
      await opaque.settle()
      row(opaque.document, 'fixture.op.7').click()
      await opaque.settle()
      buttons(opaque.document, /^Send$/)[0].click()
      await opaque.settle()
      assert.match(
        opaque.document.querySelector('#play').textContent,
        /\[REDACTED\]/,
        'a response the page could not split was dropped rather than shown — the redactor already rendered it'
      )
    } finally {
      await opaque.close()
    }
  } finally {
    await page.close()
  }
})
