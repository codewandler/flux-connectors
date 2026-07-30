// The generated catalogue, loaded at build time.
//
// A VitePress data loader rather than a `fetch()` in the browser, for two reasons. The pages are
// pre-rendered against it, so an operation's content is in the static HTML and survives with
// JavaScript switched off; and a missing or malformed document fails the site build instead of
// rendering an empty explorer at runtime.
//
// The file itself still ships at `/catalog.json` — it lives in `public/`, which VitePress serves
// verbatim — so it stays fetchable by anything else that wants it.

import { readFileSync } from 'node:fs'
import { defineLoader } from 'vitepress'
import type { Catalog } from './catalog.mts'

declare const data: Catalog
export { data }

export default defineLoader({
  // Resolved against this file's directory, and watched: editing the document reloads the dev
  // server. `cargo run -p connector-cli -- build` is what writes it.
  watch: ['../public/catalog.json'],

  load(watched: string[]): Catalog {
    const [source] = watched
    if (!source) {
      throw new Error(
        'web/public/catalog.json is missing — the site has no catalogue to read. ' +
          'Run `cargo run -p connector-cli -- build`.'
      )
    }
    return JSON.parse(readFileSync(source, 'utf-8')) as Catalog
  },
})
