// One page per operation, enumerated from the generated catalogue.
//
// This is what makes an operation deep-linkable: `/operations/<id>` is a real, pre-rendered page
// rather than a client-side route, so the URL survives being pasted into an issue, opens without
// JavaScript, and is indexable. Add an operation to a provider TOML and its page appears; remove one
// and the page goes with it. There is no list of operations anywhere in this directory.

import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import type { Catalog } from '../data/catalog.mts'

const source = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  'public',
  'catalog.json'
)

export default {
  paths() {
    const catalog = JSON.parse(readFileSync(source, 'utf-8')) as Catalog

    return catalog.providers.flatMap((provider) =>
      provider.operations.map((operation) => ({
        params: { operation: operation.id, provider: provider.id },
        // Injected at the `<!-- @content -->` marker in `[operation].md`. It is the page's `<h1>`,
        // which is also where VitePress takes the document title from — so a shared link shows the
        // operation's own name rather than the site's.
        content: `# ${operation.id}\n`,
      }))
    )
  },
}
