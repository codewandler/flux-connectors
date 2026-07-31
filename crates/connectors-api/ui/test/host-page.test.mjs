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
    calls.push({ url, method: init.method ?? 'GET' })
    const body = url === '/auth/status' ? status : (routes[url] ?? null)
    return {
      ok: true,
      status: 200,
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
