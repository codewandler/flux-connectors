// The catalogue is a release asset, and that is a gate rather than a habit (C-547).
//
// `crates/catalog-reader/README.md` tells a consumer with no Rust toolchain to fetch
// `catalog.pack` and `catalog.pack.sha256` from a `vX.Y.Z` release, check the digest out of band,
// and hand the file to `Pack::load`. Every word of that is a promise about a workflow: that some
// workflow a tag push triggers attaches both assets, that the bytes are the pack the tag records
// rather than whatever a branch holds, and that a pack disagreeing with the tag's `connectors.lock`
// is refused instead of published. v0.22.0's assets were attached by hand, which is exactly the
// state this pins shut — a hand-attached asset is indistinguishable, at the URL, from a mechanical
// one, right up until the release nobody remembered to do it for.
//
// So this file asserts the wiring rather than the pack: `ci_gate.test.mjs`'s trade, for the same
// reason. Each property below is one a future edit can narrow away silently — dropping the
// `.sha256`, letting the checkout default to a branch, softening the digest check to a warning —
// and none of those narrowings is visible in the artefact they damage.
//
// The workflow reader here is `ci_gate.test.mjs`'s, plus `uses:` and a step's `ref:`. It is
// duplicated rather than shared: that file exports nothing, the site has exactly one dependency and
// this adds none, and a 60-line reader for the slice of YAML two tests ask about is cheaper than a
// third module both must import. If a third file ever needs it, that is when it moves.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const repoRoot = path.resolve(webRoot, '..')
const workflowDir = path.join(repoRoot, '.github', 'workflows')
const readerReadme = path.join(repoRoot, 'crates', 'catalog-reader', 'README.md')

/** The two asset names the README's fetch recipe names, and the workflow must attach. */
const ASSETS = ['catalog.pack', 'catalog.pack.sha256']

/** The indentation of a line, counting spaces only — workflows here are space-indented. */
function indentOf(line) {
  return line.length - line.trimStart().length
}

/** True for a line that carries no key and no item: blank, or comment-only. */
function isNoise(line) {
  const trimmed = line.trim()
  return trimmed === '' || trimmed.startsWith('#')
}

/** The lines of the block introduced by a top-level `key:`, up to the next top-level key. */
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
 * The steps of each job, as `{ workflow, job, steps: [{ name, uses, run, ref }] }`.
 *
 * `ref` is read wherever it appears in a step, which for `actions/checkout` is under `with:`. A
 * `run: |` block is folded back into one string; its comment lines are dropped with every other
 * comment, so a property asserted below has to be in the script rather than about it.
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
      step = { name: null, uses: null, run: null, ref: null }
      current.steps.push(step)
      readStepKey(step, stepStart[1], 8)
      continue
    }
    if (step && indent >= 8) readStepKey(step, line.trim(), indent)
  }
  if (pending !== null && step) step.run = step.run.trim()

  return jobs

  function readStepKey(target, text, keyIndent) {
    const pair = text.match(/^(run|uses|name|ref):\s*(.*)$/)
    if (!pair) return
    const [, key, rawValue] = pair
    const value = rawValue.trim().replace(/^['"]|['"]$/g, '')
    if (key !== 'run') {
      target[key] = value
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

/** The tag patterns a workflow filters its push trigger by, `[]` when it filters by none. */
function tagPatternsOf(workflowName) {
  const lines = readFileSync(path.join(workflowDir, workflowName), 'utf-8').split('\n')
  const body = topLevelBlock(lines, 'on') ?? []
  const tags = body.findIndex((line) => /^\s+tags:\s*$/.test(line))
  if (tags === -1) return []
  const patterns = []
  for (const line of body.slice(tags + 1)) {
    if (isNoise(line)) continue
    const item = line.match(/^\s+- (.*)$/)
    if (!item) break
    patterns.push(item[1].trim().replace(/^['"]|['"]$/g, ''))
  }
  return patterns
}

/**
 * True when `text` names `asset` as itself rather than as the head of a longer name.
 *
 * `catalog.pack` is a prefix of `catalog.pack.sha256`, so a plain `includes` reports the pack as
 * uploaded by a command that uploads only the digest.
 */
function mentionsAsset(text, asset) {
  return new RegExp(`${asset.replace(/\./g, '\\.')}(?![\\w.])`).test(text)
}

/**
 * The `gh release upload` command of a step, with backslash continuations folded in — or `null`.
 *
 * Asked of the *command* and not of the step, deliberately: seeded on 2026-08-12 by deleting the
 * `.sha256` argument from the upload, the whole-step version of this stayed green, because the
 * verification loop further down the same script still named the file. A step that mentions an
 * asset is not a step that attaches one.
 */
function uploadCommandOf(step) {
  if (step.run === null) return null
  const lines = step.run.split('\n')
  const start = lines.findIndex((line) => line.includes('gh release upload'))
  if (start === -1) return null
  let command = lines[start]
  for (let i = start + 1; command.endsWith('\\') && i < lines.length; i += 1) {
    command = `${command.slice(0, -1)} ${lines[i]}`
  }
  return command
}

/** A step is the attachment when its upload command carries both assets. */
function attachesThePack(step) {
  const command = uploadCommandOf(step)
  return command !== null && ASSETS.every((asset) => mentionsAsset(command, asset))
}

/** A step is the crates.io publish when it runs the publish script or `cargo publish`. */
function publishesToCratesIo(step) {
  return step.run !== null && /publish-crates-io\.sh|cargo publish/.test(step.run)
}

/**
 * The jobs that attach the pack — asserted non-empty, because every question below is vacuous
 * when nothing attaches anything, and a vacuous pass is the failure mode this file exists for.
 */
function attachingJobs() {
  const jobs = allJobs().filter((job) => job.steps.some(attachesThePack))
  assert.ok(
    jobs.length > 0,
    `no job in .github/workflows uploads ${ASSETS.join(' and ')} to a release — the README's fetch URL is a promise nothing keeps, and the assets are back to being attached by hand`
  )
  return jobs
}

test('the pack and its digest are attached by a workflow a version tag triggers', () => {
  for (const job of attachingJobs()) {
    // Not merely "some workflow": an attachment on any other trigger is one someone has to
    // remember to fire, which is the hand-attachment this story replaces wearing a workflow's hat.
    assert.ok(
      triggersOf(job.workflow).includes('push'),
      `${job.workflow}:${job.job} attaches the pack but no push triggers ${job.workflow} — a release tag would not fire it`
    )
    const patterns = tagPatternsOf(job.workflow)
    assert.ok(
      patterns.some((pattern) => /^v\[0-9\]\+\.\[0-9\]\+\.\[0-9\]\+$/.test(pattern)),
      `${job.workflow} filters its push trigger by ${patterns.join(', ') || '(no tags)'} — a vX.Y.Z tag must be one of them, or the release it cuts carries no catalogue`
    )
  }
})

test('the attached bytes are the pack the tag records, never a branch head', () => {
  for (const job of attachingJobs()) {
    const checkouts = job.steps.filter((step) => (step.uses ?? '').startsWith('actions/checkout@'))
    assert.ok(
      checkouts.length > 0,
      `${job.workflow}:${job.job} uploads the pack without checking anything out`
    )
    // The default checkout is the ref that triggered the run, which is the tag for a tag push and
    // a *branch* for a `workflow_dispatch` or a `release` event. The asset has to be byte-identical
    // to the committed pack at the tag — a release whose catalogue is whatever main held that
    // afternoon is worse than no asset, because the digest check would still pass.
    for (const checkout of checkouts) {
      assert.ok(
        (checkout.ref ?? '').includes('refs/tags/'),
        `${job.workflow}:${job.job} checks out \`${checkout.ref ?? '(the default ref)'}\` — the pack it attaches must come from refs/tags/<tag>, not from a branch`
      )
    }
  }
})

test("the workflow refuses a pack the tag's connectors.lock does not vouch for", () => {
  for (const job of attachingJobs()) {
    const checking = job.steps.filter(
      (step) =>
        step.run !== null && step.run.includes('connectors.lock') && step.run.includes('sha256sum')
    )
    assert.ok(
      checking.length > 0,
      `${job.workflow}:${job.job} attaches the pack without hashing it against the tag's connectors.lock [pack] row — the .sha256 asset would then only agree with the file it was computed from`
    )
    // Loudly, and as a failure: a digest disagreement that logs a warning and uploads anyway
    // publishes a catalogue the repository does not vouch for, under a digest that looks checked.
    for (const step of checking) {
      assert.match(
        step.run,
        /::error::/,
        `${job.workflow}:${job.job}'s digest check does not report a disagreement as an error annotation`
      )
      assert.match(
        step.run,
        /exit 1/,
        `${job.workflow}:${job.job}'s digest check does not exit non-zero on a disagreement — it warns and attaches`
      )
    }
  }
})

test('attaching the pack cannot fail the crates.io publish', () => {
  for (const job of attachingJobs()) {
    // A published version cannot be withdrawn (AGENTS.md § Publishing contract), so the irreversible
    // step does not get a new way to go red. Sharing a job is what would give it one.
    assert.ok(
      !job.steps.some(publishesToCratesIo),
      `${job.workflow}:${job.job} both publishes to crates.io and attaches the pack — a failed attachment would then fail a publish that cannot be re-run`
    )
  }
})

test('a release object created after the tag push still gets the assets', () => {
  for (const job of attachingJobs()) {
    // The ordering is real: AGENTS.md § Release process creates the release only once the publish
    // is green, so the tag-push run usually finds no release to upload to. The second trigger is
    // what makes "every release carries the pack" true rather than "every release created before
    // its own tag was pushed".
    assert.ok(
      triggersOf(job.workflow).includes('release'),
      `${job.workflow} attaches the pack on a tag push only, and the release object is created after the publish is green — that run finds no release and the assets are never attached`
    )
    const upload = uploadCommandOf(job.steps.find(attachesThePack))
    assert.match(
      upload,
      /--clobber/,
      `${job.workflow}:${job.job} uploads without --clobber — the second of the two triggers, or any re-run, then fails on the asset name instead of replacing identical bytes`
    )
  }
})

test('the reader README documents the assets the workflow actually attaches', () => {
  const readme = readFileSync(readerReadme, 'utf-8')
  const upload = uploadCommandOf(attachingJobs()[0].steps.find(attachesThePack))

  // The link this test exists to keep: the consumer contract is written in a README on crates.io,
  // and the thing that honours it is a workflow in this repository. Either can be edited alone.
  for (const asset of ASSETS) {
    assert.ok(
      mentionsAsset(readme, asset),
      `the reader README does not name \`${asset}\`, which the workflow attaches — a consumer reads the README, not the workflow`
    )
    assert.ok(
      mentionsAsset(upload, asset),
      `the workflow does not attach \`${asset}\`, which the reader README tells consumers to fetch`
    )
  }
  for (const claim of [
    'releases/download/', // the URL shape a fetch without a clone needs
    'sha256sum -c', // the out-of-band check, in the format the .sha256 asset is written in
    'connectors.lock', // where the out-of-band digest comes from, so it is the repository's number
    'Pack::load', // the in-band half
    'UnsupportedSchema', // …and its refusals, by name, because that is how a consumer reads them
    'DigestMismatch',
  ]) {
    assert.ok(
      readme.includes(claim),
      `the reader README no longer documents \`${claim}\` — the fetch-and-load contract is only as good as the half a consumer can read`
    )
  }
})
