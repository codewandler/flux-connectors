// The site's gate is a gate, not a habit (C-240).
//
// `explorer.test.mjs` next door is 32 assertions over the built site, and until this story it ran in
// no workflow at all — `.github/workflows/pages.yml` ran `npm ci` and `npm run build` and stopped,
// and `ci.yml` was Rust-only. It was green the whole time, which is the point: nothing would have
// told us otherwise. `AGENTS.md` meanwhile documented `npm ci && npm run build && npm test` as *the*
// web gate, so the repository already believed this ran.
//
// So this file asserts the wiring rather than the site: that some workflow a pull request triggers
// runs the suite, that it builds before it tests, and that the gate AGENTS.md documents is the gate
// the workflow enforces. Each of those is a property a future edit can quietly drop, and the drop is
// invisible in exactly the way the original defect was.
//
// One honest limitation: this test rides in the suite it guards, so deleting the web job from
// `ci.yml` also stops this file running *in CI*. It still fails for anyone running the gate AGENTS.md
// documents, and the deletion is a visible diff in a 90-line workflow. A non-circular version would
// live in the Rust gate; that is a bigger surface than this story owns.
//
// Node's built-in test runner and a hand-rolled reader for the slice of YAML a workflow uses — the
// site has exactly one dependency and this adds none, the same trade `SchemaBlock.vue` makes for
// syntax highlighting.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const repoRoot = path.resolve(webRoot, '..')
const workflowDir = path.join(repoRoot, '.github', 'workflows')

/** The indentation of a line, counting spaces only — workflows here are space-indented. */
function indentOf(line) {
  return line.length - line.trimStart().length
}

/** True for a line that carries no key and no item: blank, or comment-only. */
function isNoise(line) {
  const trimmed = line.trim()
  return trimmed === '' || trimmed.startsWith('#')
}

/**
 * The lines of the block introduced by a top-level `key:`, up to the next top-level key.
 *
 * Returns `null` when the key is absent, which is a different answer from an empty block and is
 * worth keeping distinct: a workflow with no `on:` is malformed, one with an empty `on:` is a
 * statement.
 */
function topLevelBlock(lines, key) {
  const start = lines.findIndex((line) => line.startsWith(`${key}:`))
  if (start === -1) return null
  const body = []
  for (const line of lines.slice(start + 1)) {
    if (!isNoise(line) && indentOf(line) === 0) break
    body.push(line)
  }
  return body
}

/**
 * The steps of each job, as `{ workflow, job, steps: [{ run, workingDirectory }] }`.
 *
 * Only what this file asks about is modelled: a step's `run` and its `working-directory`. A `run: |`
 * block is folded back into one string so an ordering question can be asked of it either way.
 */
function jobsOf(workflowPath) {
  const lines = readFileSync(workflowPath, 'utf-8').split('\n')
  const body = topLevelBlock(lines, 'jobs') ?? []
  const jobs = []

  let current = null
  let step = null
  let pending = null // the indent of the `run: |` scalar still being collected, if any

  for (const line of body) {
    const indent = indentOf(line)

    // A block scalar continues while its lines stay indented past the key that introduced it.
    if (pending !== null) {
      if (isNoise(line)) continue
      if (indent > pending) {
        step.run += `${step.run ? '\n' : ''}${line.trim()}`
        continue
      }
      step.run = step.run.trim()
      pending = null
    }
    if (isNoise(line)) continue

    const jobHeader = line.match(/^ {2}([A-Za-z0-9_.-]+):\s*$/)
    if (jobHeader) {
      current = { workflow: path.basename(workflowPath), job: jobHeader[1], steps: [] }
      jobs.push(current)
      step = null
      continue
    }
    if (!current) continue

    // A step begins at `      - `; every later key of that step is indented past the dash.
    const stepStart = line.match(/^ {6}- (.*)$/)
    if (stepStart) {
      step = { run: null, workingDirectory: null }
      current.steps.push(step)
      readStepKey(step, stepStart[1], 8)
      continue
    }
    if (step && indent >= 8) readStepKey(step, line.trim(), indent)
  }
  if (pending !== null && step) step.run = step.run.trim()

  return jobs

  function readStepKey(target, text, keyIndent) {
    const pair = text.match(/^(run|working-directory):\s*(.*)$/)
    if (!pair) return
    const [, key, rawValue] = pair
    const value = rawValue.trim()
    if (key === 'working-directory') {
      target.workingDirectory = value.replace(/^['"]|['"]$/g, '')
      return
    }
    if (value === '|' || value === '>') {
      target.run = ''
      pending = keyIndent
      return
    }
    target.run = value
  }
}

/** Every job of every workflow in `.github/workflows`. */
function allJobs() {
  return readdirSync(workflowDir)
    .filter((name) => name.endsWith('.yml') || name.endsWith('.yaml'))
    .flatMap((name) => jobsOf(path.join(workflowDir, name)))
}

/** The events a workflow declares under `on:`. */
function triggersOf(workflowName) {
  const lines = readFileSync(path.join(workflowDir, workflowName), 'utf-8').split('\n')
  const body = topLevelBlock(lines, 'on')
  assert.ok(body, `${workflowName} declares no \`on:\` block, so nothing triggers it`)
  return body
    .filter((line) => !isNoise(line) && indentOf(line) === 2)
    .map((line) => line.trim().replace(/:.*$/, ''))
}

/** A step is "the web suite" when it runs `npm test` from `web/`. */
function runsTheWebSuite(step) {
  return step.run !== null && /(^|\s)npm test(\s|$)/.test(step.run) && step.workingDirectory === 'web'
}

/** A step is the site build when it runs `npm run build` from `web/`. */
function buildsTheSite(step) {
  return step.run !== null && step.run.includes('npm run build') && step.workingDirectory === 'web'
}

test('the explorer suite runs in a workflow that a pull request triggers', () => {
  const running = allJobs().filter((job) => job.steps.some(runsTheWebSuite))

  assert.ok(
    running.length > 0,
    'no job in .github/workflows runs `npm test` in `web/` — the explorer suite guards nothing a push can see'
  )

  // The reason this asks for `pull_request` and not merely "some workflow": a suite that runs only
  // on the deploy path reports a broken site *after* the merge that broke it.
  const onPullRequests = running.filter((job) => triggersOf(job.workflow).includes('pull_request'))
  assert.ok(
    onPullRequests.length > 0,
    `the explorer suite runs only in ${running
      .map((job) => `${job.workflow}:${job.job}`)
      .join(', ')}, and no pull request triggers it — it cannot block a bad merge`
  )
})

test('the site is built before it is tested, in the same job', () => {
  for (const job of allJobs()) {
    const test_ = job.steps.findIndex(runsTheWebSuite)
    if (test_ === -1) continue
    const build = job.steps.findIndex(buildsTheSite)

    // The suite asserts against `.vitepress/dist`, so this is not a stylistic ordering. Measured on
    // 2026-08-01 at cecf320 by running `npm test` in a worktree with no build: 13 pass, **19 fail**,
    // every one of them a page that was never rendered.
    assert.notEqual(
      build,
      -1,
      `${job.workflow}:${job.job} runs \`npm test\` without \`npm run build\` — the suite reads .vitepress/dist and reports 19 failures against a site that was never built`
    )
    assert.ok(
      build < test_,
      `${job.workflow}:${job.job} runs \`npm test\` at step ${test_ + 1} but builds at step ${build + 1} — the suite reads .vitepress/dist, so it must run after the build`
    )
  }
})

test("AGENTS.md's web gate names the workflow that enforces it, and that workflow runs it", () => {
  const agents = readFileSync(path.join(repoRoot, 'AGENTS.md'), 'utf-8')

  // The defect this story fixes was documentation running ahead of enforcement: AGENTS.md described
  // this gate for months while no workflow ran it. Naming the workflow is what makes the drift
  // checkable rather than a matter of someone remembering.
  const named = [...agents.matchAll(/\.github\/workflows\/([A-Za-z0-9_.-]+\.ya?ml)/g)].map((m) => m[1])
  const enforcing = allJobs().filter((job) => job.steps.some(runsTheWebSuite))

  assert.ok(
    enforcing.some((job) => named.includes(job.workflow)),
    `AGENTS.md's web gate does not name a workflow that runs it; the suite runs in ${
      enforcing.map((job) => job.workflow).join(', ') || '(nothing)'
    }`
  )

  for (const command of ['npm ci', 'npm run build', 'npm test']) {
    assert.ok(agents.includes(command), `AGENTS.md's web gate no longer documents \`${command}\``)
  }
})
