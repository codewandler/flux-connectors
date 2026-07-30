import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { allCoreEntries, type Catalog } from '../../data/catalog.mts'

const source = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'public',
  'catalog.json'
)

export default {
  paths() {
    const catalog = JSON.parse(readFileSync(source, 'utf-8')) as Catalog
    if (!catalog.core) return []

    return allCoreEntries(catalog.core).map((entry) => {
      const kind = entry.kind === 'capability' ? 'capabilities' : `${entry.kind}s`
      return {
        params: { kind, name: entry.name },
        content: `# ${entry.title}\n`,
      }
    })
  },
}
